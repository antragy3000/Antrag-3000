// Baut aus Stammdaten + Sammel-Formular + Förderung die Inhalte
// für antworten.json (Quelle der Wahrheit) und die Word-Datei.
// Liegt bewusst im Frontend: WAS im Antrag steht, ist Anwendungs-
// logik - Rust setzt es nur noch in eine .docx-Datei um.

import { fristText } from "./begriffe";
import { kfpAbschnitte, kfpExport } from "./kfp";

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

function zeilen(...teile) {
  return teile.filter((t) => t && t.trim() !== "");
}

export function antragBauen(stammdaten, formular, foerderung, kfp) {
  const heute = new Date().toLocaleDateString("de-DE");
  const s = stammdaten;

  const warnhinweis =
    `AUTOMATISCH ERZEUGT – Antrag 3000, ${heute}\n` +
    `Verbindliche Quelle ist die App (bzw. die Datei antworten.json in diesem Ordner).\n` +
    `Änderungen in dieser Word-Datei werden NICHT automatisch in die App übernommen.`;

  const titel = `Antrag: ${formular.projekttitel || "(ohne Projekttitel)"} – ${foerderung.name}`;

  const abschnitte = [];

  abschnitte.push({
    ueberschrift: "Förderung",
    absaetze: zeilen(
      `${foerderung.name} – ${foerderung.foerdergeber}`,
      `Förderhöhe: ${foerderung.foerderhoehe_text}`,
      fristText(foerderung),
      `Webseite: ${foerderung.webseite}`
    ),
  });

  const name = zeilen(
    [s.person.vorname, s.person.nachname].filter(Boolean).join(" "),
    s.person.kuenstlername && `Künstler:innenname: ${s.person.kuenstlername}`,
    s.person.organisation && `Organisation/Träger: ${s.person.organisation}`
  );
  const kontakt = zeilen(
    s.kontakt.strasse,
    [s.kontakt.plz, s.kontakt.ort].filter(Boolean).join(" "),
    s.kontakt.land,
    s.kontakt.email && `E-Mail: ${s.kontakt.email}`,
    s.kontakt.telefon && `Telefon: ${s.kontakt.telefon}`,
    s.kontakt.webseite && `Webseite: ${s.kontakt.webseite}`
  );
  abschnitte.push({
    ueberschrift: "Antragsteller:in",
    absaetze: name.concat(kontakt),
  });

  const bank = zeilen(
    s.bank.kontoinhaber && `Kontoinhaber:in: ${s.bank.kontoinhaber}`,
    s.bank.iban && `IBAN: ${s.bank.iban}`,
    s.bank.bic && `BIC: ${s.bank.bic}`,
    s.bank.bank && `Bank: ${s.bank.bank}`
  );
  if (bank.length) abschnitte.push({ ueberschrift: "Bankverbindung", absaetze: bank });

  const steuer = zeilen(
    s.steuer.steuernummer && `Steuernummer: ${s.steuer.steuernummer}`,
    s.steuer.ustid && `USt-IdNr.: ${s.steuer.ustid}`,
    s.steuer.finanzamt && `Finanzamt: ${s.steuer.finanzamt}`
  );
  if (steuer.length) abschnitte.push({ ueberschrift: "Steuerliche Angaben", absaetze: steuer });

  // Inhaltliche Felder aus dem Sammel-Formular (leere überspringen).
  for (const [key, beschriftung] of FORMULAR_FELDER) {
    if (key === "projekttitel") continue; // steht schon im Titel
    const wert = (formular[key] || "").trim();
    if (wert) abschnitte.push({ ueberschrift: beschriftung, absaetze: [wert] });
  }

  // Kostenfinanzplan als Tabellen (Kosten, Finanzierung, Bilanz).
  for (const a of kfpAbschnitte(kfp)) abschnitte.push(a);

  abschnitte.push({
    ueberschrift: "Übliche Unterlagen (Checkliste des Fördergebers)",
    absaetze: foerderung.checkliste_vorschlag.map((p) => `☐  ${p}`),
  });

  const antwortenJson = JSON.stringify(
    {
      schema_version: 1,
      erzeugt: new Date().toISOString(),
      hinweis:
        "Maschinenlesbare Quelle der Wahrheit, erzeugt von Antrag 3000. Nicht von Hand bearbeiten.",
      foerderung: {
        id: foerderung.id,
        name: foerderung.name,
        foerdergeber: foerderung.foerdergeber,
      },
      formular,
      kfp: kfpExport(kfp),
      stammdaten: s,
    },
    null,
    2
  );

  return { titel, warnhinweis, abschnitte, antwortenJson };
}
