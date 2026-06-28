// Baut die Abschnitte für das Antrags-PDF zusammen (Stammblatt,
// Formular, Kostenfinanzplan, Anhang-Liste) sowie die Liste der
// hochgeladenen Anhänge. WAS im PDF steht, ist Anwendungslogik und
// liegt deshalb hier im Frontend; Rust (pdf.rs) rendert nur und hängt
// die Anhänge an.

import { FORMULAR_FELDER } from "./antrag";
import { fristText } from "./begriffe";
import { kfpAbschnitteFuerAntrag } from "./kfp";

function zeilen(...teile) {
  return teile.filter((t) => t && String(t).trim() !== "");
}

export function antragsPdfBauen(stammdaten, formular, kfp, foerderung, checkliste) {
  const s = stammdaten;
  const titel = `Förderantrag: ${formular.projekttitel || "(ohne Projekttitel)"} – ${foerderung.name}`;
  const abschnitte = [];

  // 1. Stammblatt
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
  abschnitte.push({ ueberschrift: "Antragsteller:in", absaetze: name.concat(kontakt), tabelle: [] });

  const bank = zeilen(
    s.bank.kontoinhaber && `Kontoinhaber:in: ${s.bank.kontoinhaber}`,
    s.bank.iban && `IBAN: ${s.bank.iban}`,
    s.bank.bic && `BIC: ${s.bank.bic}`,
    s.bank.bank && `Bank: ${s.bank.bank}`
  );
  if (bank.length) abschnitte.push({ ueberschrift: "Bankverbindung", absaetze: bank, tabelle: [] });

  const steuer = zeilen(
    s.steuer.steuernummer && `Steuernummer: ${s.steuer.steuernummer}`,
    s.steuer.ustid && `USt-IdNr.: ${s.steuer.ustid}`,
    s.steuer.finanzamt && `Finanzamt: ${s.steuer.finanzamt}`
  );
  if (steuer.length) abschnitte.push({ ueberschrift: "Steuerliche Angaben", absaetze: steuer, tabelle: [] });

  // Förderung, auf die sich der Antrag bezieht
  abschnitte.push({
    ueberschrift: "Förderung",
    absaetze: zeilen(
      `${foerderung.name} – ${foerderung.foerdergeber}`,
      `Förderhöhe: ${foerderung.foerderhoehe_text}`,
      fristText(foerderung),
      foerderung.webseite && `Webseite: ${foerderung.webseite}`
    ),
    tabelle: [],
  });

  // 2. Daten aus dem Formular
  for (const [key, beschriftung] of FORMULAR_FELDER) {
    if (key === "projekttitel") continue; // steht schon im Titel
    const wert = (formular[key] || "").trim();
    if (wert) abschnitte.push({ ueberschrift: beschriftung, absaetze: [wert], tabelle: [] });
  }

  // 3. Kostenfinanzplan (auf diese Förderung zugeschnitten:
  //    andere Mittel + zu beantragende Summe als Fehlbetrag = der für
  //    diesen Förderer im KFP eingeplante Betrag)
  for (const a of kfpAbschnitteFuerAntrag(kfp, foerderung.id, foerderung.name)) {
    abschnitte.push({
      ueberschrift: a.ueberschrift,
      absaetze: a.absaetze ?? [],
      tabelle: a.tabelle ?? [],
    });
  }

  // 4. Anhang-Liste (Titel der benötigten Dokumente)
  const liste = (checkliste ?? []).map((p, i) => `${i + 1}. ${p.text}`);
  abschnitte.push({
    ueberschrift: "Anhang",
    absaetze: liste.length
      ? ["Diesem Antrag sind folgende Dokumente beigefügt:", ...liste]
      : ["Keine Anhänge."],
    tabelle: [],
  });

  // Hochgeladene Dateien in Reihenfolge der Anhang-Liste.
  const anhaenge = (checkliste ?? [])
    .map((p) => (p.datei || "").trim())
    .filter(Boolean);

  return { titel, abschnitte, anhaenge };
}
