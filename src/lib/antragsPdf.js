// Baut die Abschnitte für das Antrags-PDF zusammen (Stammblatt,
// Formular, Kostenfinanzplan, Anhang-Liste) sowie die Liste der
// hochgeladenen Anhänge. WAS im PDF steht, ist Anwendungslogik und
// liegt deshalb hier im Frontend; Rust (pdf.rs) rendert nur und hängt
// die Anhänge an.

import { FORMULAR_FELDER } from "./antrag";
import { kfpAbschnitteFuerAntrag } from "./kfp";

function zeilen(...teile) {
  return teile.filter((t) => t && String(t).trim() !== "");
}

export function antragsPdfBauen(stammdaten, formular, kfp, foerderung, checkliste) {
  const s = stammdaten;
  const titel = `Förderantrag: ${formular.projekttitel || "(ohne Projekttitel)"} – ${foerderung.name}`;
  const abschnitte = [];

  // 1. Absender-Briefkopf: Name + Kontakt der antragstellenden Person/
  //    Organisation. Steht oben im PDF (rechtsbündig, neben/unter dem Logo)
  //    wie der Absender auf einem Brief – NICHT mehr als Abschnitt im Text.
  const absender = zeilen(
    [s.person.vorname, s.person.nachname].filter(Boolean).join(" "),
    s.person.kuenstlername && `Künstler:innenname: ${s.person.kuenstlername}`,
    s.person.organisation && `Organisation/Träger: ${s.person.organisation}`,
    s.kontakt.strasse,
    [s.kontakt.plz, s.kontakt.ort].filter(Boolean).join(" "),
    s.kontakt.land,
    s.kontakt.email && `E-Mail: ${s.kontakt.email}`,
    s.kontakt.telefon && `Telefon: ${s.kontakt.telefon}`,
    s.kontakt.webseite && `Webseite: ${s.kontakt.webseite}`
  );

  // Finanzangaben (Bank + Steuer) werden hier nur GEBAUT und erst NACH dem
  // Kostenfinanzplan eingefügt – so stehen alle Geld-/Verwaltungsangaben
  // zusammen am Ende des Antrags (kein Absender-Material).
  const bank = zeilen(
    s.bank.kontoinhaber && `Kontoinhaber:in: ${s.bank.kontoinhaber}`,
    s.bank.iban && `IBAN: ${s.bank.iban}`,
    s.bank.bic && `BIC: ${s.bank.bic}`,
    s.bank.bank && `Bank: ${s.bank.bank}`
  );
  const steuer = zeilen(
    s.steuer.steuernummer && `Steuernummer: ${s.steuer.steuernummer}`,
    s.steuer.ustid && `USt-IdNr.: ${s.steuer.ustid}`,
    s.steuer.finanzamt && `Finanzamt: ${s.steuer.finanzamt}`
  );

  // (Der Förderer/die Förderung steht bereits im Titel „Förderantrag: … – …",
  //  daher kein eigener „Förderung"-Abschnitt mehr.)

  // 2. Daten aus dem Formular
  for (const [key, beschriftung] of FORMULAR_FELDER) {
    if (key === "projekttitel") continue; // steht schon im Titel
    const wert = (formular[key] || "").trim();
    if (wert) abschnitte.push({ ueberschrift: beschriftung, absaetze: [wert], tabelle: [] });
  }

  // Benutzerdefinierte Zusatzfelder anhaengen.
  for (const feld of formular._eigeneFelder ?? []) {
    const ueberschrift = (feld.beschriftung || "").trim();
    const wert = (feld.wert || "").trim();
    if (ueberschrift && wert) abschnitte.push({ ueberschrift, absaetze: [wert], tabelle: [] });
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

  // Finanzangaben NACH dem Kostenfinanzplan.
  if (bank.length) abschnitte.push({ ueberschrift: "Bankverbindung", absaetze: bank, tabelle: [] });
  if (steuer.length) abschnitte.push({ ueberschrift: "Steuerliche Angaben", absaetze: steuer, tabelle: [] });

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

  return { titel, absender, abschnitte, anhaenge };
}
