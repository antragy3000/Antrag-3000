// ============================================================
// Ausgaben-Test: prüft, was eine neue Ausgabe von der vorigen übernimmt und vor
// allem, was sie NIE übernimmt – ohne die App zu starten.
//
// Ausfuehren:  npm run ausgaben   (oder: node tools/ausgaben-test.mjs)
// ============================================================

import {
  ausgabeNameVorschlag,
  gruppenNameVorschlag,
  ausgabeBefuellen,
  vorigesErgebnis,
} from "../src/lib/ausgaben.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

// Unverwechselbare Markierungen für alles, was NIE mitkommen darf.
const NIE = {
  beleg: "NIE_BELEG_Baumarkt",
  quelle: "NIE_GELDQUELLE_Stiftung",
  sachbericht: "NIE_SACHBERICHT_Text",
  datei: "NIE_DATEI_Lebenslauf.pdf",
  eigeneFrist: "NIE_EIGENE_FRIST_Kickoff",
  interneFrist: "NIE_INTERNE_FRIST_Treffen",
  dokLink: "https://cryptpad.fr/pad/NIE_DOKUMENT_LINK",
  ablage: "https://cryptpad.fr/drive/NIE_ABLAGE_LINK",
  fragebogen: "NIE_FRAGEBOGEN_Budget",
  statusFrei: "NIE_FREIER_STATUS",
  checkFrei: "NIE_FREIER_CHECK_STATUS",
  altFrist: "1999-01-01",
  eigenFrist: "1999-02-02",
  ghostFrist: "1999-03-03",
  zeitraum: "NIE_ZEITRAUM_2026",
};

function vorlage() {
  return {
    id: "alt",
    name: "Festival 2026",
    gruppeId: "g1",
    archiviert: false,
    fragebogen: { budget: NIE.fragebogen },
    formular: {
      projekttitel: "Klangraum",
      beschreibung: "Ein Festival",
      zeitraum: NIE.zeitraum,
      ort: "Berlin",
      _eigeneFelder: [{ id: "e1", beschriftung: "Barrierefreiheit", typ: "textarea", wert: "Rampe" }],
    },
    kfp: {
      kosten: [{ name: "Honorare", posten: [{ id: "k1", bezeichnung: "Gagen", erlaeuterung: "5 Personen", betrag: "50 × 4" }] }],
      finanzierung: [{
        name: "",
        posten: [
          { bezeichnung: "Eigenmittel", betrag: "1.000", foerderId: "" },
          { bezeichnung: "", betrag: "2.000", foerderId: "kat-1" },
        ],
      }],
    },
    merkliste: ["kat-1", "kat-ohne-antrag", "eigen-1", "ghost-1"],
    eigeneFoerderungen: [
      { id: "eigen-1", eigen: true, name: "Eigene", fristen: [NIE.eigenFrist] },
      { id: "eigen-weg", eigen: true, name: "Nicht mehr gemerkt", fristen: [] },
    ],
    katalogGhosts: [
      { id: "ghost-1", name: "Eingestellt", nichtMehrImKatalog: true, fristen: [NIE.ghostFrist] },
      { id: "ghost-weg", name: "Nicht mehr gemerkt", nichtMehrImKatalog: true },
    ],
    katalogAktualisiert: ["kat-1"],
    antraege: {
      "kat-1": {
        status: "zusage",
        statusFrei: NIE.statusFrei,
        offizielleFristen: [NIE.altFrist],
        eigeneFristen: [{ datum: "2026-05-01", titel: NIE.eigeneFrist }],
        kontakt: { ansprechpartner: "Frau Berg", email: "berg@example.invalid", telefon: "", notiz: "nett" },
        checkliste: [{ text: "Lebenslauf", status: "abgeschlossen", statusFrei: NIE.checkFrei, datei: NIE.datei }],
      },
      "eigen-1": { status: "absage", statusFrei: "", offizielleFristen: [NIE.eigenFrist], eigeneFristen: [], checkliste: [] },
      "ghost-1": { status: "abgeschickt", statusFrei: "", offizielleFristen: [NIE.ghostFrist], eigeneFristen: [], kontakt: { ansprechpartner: "Herr Alt" }, checkliste: [] },
      "nicht-gemerkt": { status: "zusage", statusFrei: "", offizielleFristen: [], eigeneFristen: [], checkliste: [] },
    },
    interneFristen: [{ id: "i1", datum: "2026-03-01", titel: NIE.interneFrist }],
    abrechnung: {
      belege: [{ id: "b1", empfaenger: NIE.beleg }],
      quellen: [{ id: "q1", name: NIE.quelle, sachbericht: NIE.sachbericht }],
      sachbericht: NIE.sachbericht,
    },
    arbeitsdokumente: [{ id: "d1", titel: "Text", url: NIE.dokLink, foerderungen: [] }],
    ablageUrl: NIE.ablage,
    cryptpadHinweisAus: true,
  };
}

// Frisches Projekt wie neuesProjekt() in tresorFormat.js.
function frisch() {
  return {
    id: "neu",
    name: "Festival 2027",
    gruppeId: null,
    vorgaengerId: null,
    archiviert: false,
    fragebogen: null,
    merkliste: [],
    formular: { projekttitel: "", beschreibung: "", zeitraum: "", ort: "", _eigeneFelder: [] },
    kfp: { kosten: [], finanzierung: [] },
    abrechnung: { belege: [], quellen: [], sachbericht: "" },
    antraege: {},
    eigeneFoerderungen: [],
    interneFristen: [],
    katalogGhosts: [],
    katalogAktualisiert: [],
    arbeitsdokumente: [],
    ablageUrl: "",
    cryptpadHinweisAus: false,
  };
}

const FRISCHE_FRIST = "2027-06-30";
const fristenFuer = (id) => (id.startsWith("kat-") ? [FRISCHE_FRIST] : ["FALSCH_KEINE_QUELLE"]);
const ALLES = { texte: true, kostenplan: true, betraege: true, merkliste: true };

console.log("Namensvorschlag:");
const liste = [{ id: "a", name: "Festival 2026" }, { id: "b", name: "Festival 2027" }];
pruefe(ausgabeNameVorschlag("Festival 2026", []) === "Festival 2027", "Jahreszahl zählt hoch");
pruefe(ausgabeNameVorschlag("Festival 2026", liste) === "Festival 2028", "belegtes Jahr wird übersprungen");
pruefe(ausgabeNameVorschlag("2026 Festival", []) === "2027 Festival", "Jahreszahl vorn bleibt vorn");
pruefe(ausgabeNameVorschlag("Festival 2025 2026", []) === "Festival 2025 2027", "die letzte Jahreszahl zählt");
pruefe(ausgabeNameVorschlag("Workshop", []) === "Workshop 2", "ohne Zahl kommt eine dazu");
pruefe(ausgabeNameVorschlag("Workshop 2", [{ id: "x", name: "Workshop 3" }]) === "Workshop 4", "Nummer zählt hoch, belegte wird übersprungen");
pruefe(ausgabeNameVorschlag("Teil 12345", []) === "Teil 12345 2", "lange Zahl gilt nicht als Nummer");
pruefe(gruppenNameVorschlag("Festival 2026") === "Festival", "Gruppenname ohne Jahreszahl");
pruefe(gruppenNameVorschlag("Festival – 2026") === "Festival", "auch ohne den Strich davor");
pruefe(gruppenNameVorschlag("Festival 2026 Berlin") === "Festival Berlin", "Jahreszahl in der Mitte fällt weg");
pruefe(gruppenNameVorschlag("2026") === "2026", "nur eine Jahreszahl bleibt stehen");
pruefe(gruppenNameVorschlag("Workshop") === "Workshop", "ohne Jahreszahl bleibt der Name");

console.log("Alles übernehmen:");
const quelle = vorlage();
const davor = JSON.stringify(quelle);
const neu = ausgabeBefuellen(frisch(), quelle, ALLES, fristenFuer);
const text = JSON.stringify(neu);

pruefe(JSON.stringify(quelle) === davor, "die vorige Ausgabe bleibt unverändert");
pruefe(neu.vorgaengerId === "alt", "verweist auf die vorige Ausgabe");
pruefe(neu.id === "neu" && neu.name === "Festival 2027", "Kennung und Name bleiben die eigenen");
for (const [art, wert] of Object.entries(NIE)) {
  pruefe(!text.includes(wert), `nie übernommen: ${art}`);
}
pruefe(neu.abrechnung.belege.length === 0 && neu.abrechnung.quellen.length === 0, "Abrechnung bleibt leer");
pruefe(neu.interneFristen.length === 0, "interne Fristen bleiben leer");
pruefe(neu.arbeitsdokumente.length === 0 && neu.ablageUrl === "", "Arbeitsdokumente und Ablageort bleiben leer");
pruefe(neu.fragebogen === null, "Fragebogen bleibt leer");
pruefe(neu.katalogAktualisiert.length === 0, "Update-Hinweise bleiben leer");
pruefe(neu.archiviert === false && neu.gruppeId === null, "Archiv-Status und Gruppe setzt der Aufrufer");

console.log("Formular:");
pruefe(neu.formular.projekttitel === "Klangraum" && neu.formular.ort === "Berlin", "Texte kommen mit");
pruefe(neu.formular.zeitraum === "", "Zeitraum bleibt leer");
pruefe(neu.formular._eigeneFelder[0].wert === "Rampe", "eigene Felder kommen mit");
neu.formular._eigeneFelder[0].wert = "geändert";
pruefe(quelle.formular._eigeneFelder[0].wert === "Rampe", "Änderung wirkt nicht auf die vorige Ausgabe");

console.log("Kostenplan:");
pruefe(neu.kfp.kosten[0].posten[0].bezeichnung === "Gagen", "Struktur kommt mit");
pruefe(neu.kfp.kosten[0].posten[0].erlaeuterung === "5 Personen", "Erläuterung kommt mit");
pruefe(neu.kfp.kosten[0].posten[0].id === "k1", "Posten behalten ihre Kennung");
pruefe(neu.kfp.kosten[0].posten[0].betrag === "50 × 4", "auf Wunsch kommen die Beträge mit");
const ohneBetrag = ausgabeBefuellen(frisch(), vorlage(), { ...ALLES, betraege: false }, fristenFuer);
pruefe(ohneBetrag.kfp.kosten[0].posten[0].betrag === "", "ohne Wunsch: Kosten ohne Betrag");
pruefe(ohneBetrag.kfp.finanzierung[0].posten.every((p) => p.betrag === ""), "ohne Wunsch: Finanzierung ohne Betrag");
pruefe(ohneBetrag.kfp.finanzierung[0].posten[1].foerderId === "kat-1", "Verknüpfung zur gemerkten Förderung bleibt");
const ohneMerk = ausgabeBefuellen(frisch(), vorlage(), { ...ALLES, merkliste: false }, fristenFuer);
pruefe(ohneMerk.kfp.finanzierung[0].posten.length === 1 && ohneMerk.kfp.finanzierung[0].posten[0].bezeichnung === "Eigenmittel", "ohne Merkliste: nur freie Finanzierungs-Zeilen");

console.log("Merkliste und Anträge:");
pruefe(neu.merkliste.join() === quelle.merkliste.join(), "Merkliste kommt vollständig mit");
pruefe(neu.eigeneFoerderungen.map((f) => f.id).join() === "eigen-1", "eigene Förderungen: nur die gemerkten");
pruefe(neu.katalogGhosts.map((f) => f.id).join() === "ghost-1", "ehemalige Katalog-Förderungen: nur die gemerkten");
pruefe(neu.merkliste.every((id) => id.startsWith("kat-") || neu.eigeneFoerderungen.some((f) => f.id === id) || neu.katalogGhosts.some((f) => f.id === id)), "kein leerer Eintrag in der Merkliste");
const a = neu.antraege["kat-1"];
pruefe(a.status === "in_bearbeitung" && a.statusFrei === "", "Status beginnt neu");
pruefe(a.offizielleFristen.join() === FRISCHE_FRIST, "offizielle Fristen kommen frisch aus dem Katalog");
pruefe(a.eigeneFristen.length === 0, "eigene Fristen bleiben leer");
pruefe(a.kontakt.ansprechpartner === "Frau Berg" && a.kontakt.email === "berg@example.invalid", "Kontakt kommt mit");
pruefe(a.checkliste.length === 1 && a.checkliste[0].text === "Lebenslauf", "Checklisten-Titel kommt mit");
pruefe(a.checkliste[0].status === "noch_nicht" && a.checkliste[0].statusFrei === "" && a.checkliste[0].datei === "", "Checkliste beginnt neu, ohne Datei");
pruefe(neu.antraege["eigen-1"].offizielleFristen.length === 0, "eigene Förderung: alte Frist fällt weg");
pruefe(neu.eigeneFoerderungen[0].fristen.length === 0, "eigene Förderung: alte Frist auch am Eintrag weg");
pruefe(neu.antraege["ghost-1"].offizielleFristen.length === 0, "nicht mehr im Katalog: alte Frist fällt weg");
pruefe(neu.antraege["ghost-1"].kontakt.ansprechpartner === "Herr Alt", "nicht mehr im Katalog: Kontakt bleibt");
pruefe(neu.antraege["eigen-1"].kontakt.ansprechpartner === "", "fehlender Kontakt wird leer ergänzt");
pruefe(!("kat-ohne-antrag" in neu.antraege), "ohne alten Antrag entsteht keiner");
pruefe(!("nicht-gemerkt" in neu.antraege), "nicht mehr gemerkter Antrag kommt nicht mit");

console.log("Nichts ausgewählt:");
const nichts = ausgabeBefuellen(frisch(), vorlage(), { texte: false, kostenplan: false, betraege: false, merkliste: false }, fristenFuer);
pruefe(nichts.formular.projekttitel === "" && nichts.kfp.kosten.length === 0, "Formular und Kostenplan bleiben leer");
pruefe(nichts.merkliste.length === 0 && Object.keys(nichts.antraege).length === 0, "Merkliste und Anträge bleiben leer");
pruefe(nichts.eigeneFoerderungen.length === 0 && nichts.katalogGhosts.length === 0, "keine eigenen oder ehemaligen Förderungen");
pruefe(nichts.vorgaengerId === "alt", "die Verknüpfung bleibt trotzdem");

console.log("Ergebnis der vorigen Ausgabe:");
const v = vorlage();
pruefe(vorigesErgebnis(v, "kat-1").label === "Zusage erhalten" && vorigesErgebnis(v, "kat-1").farbe === "gruen", "Zusage");
pruefe(vorigesErgebnis(v, "eigen-1").label === "Absage erhalten" && vorigesErgebnis(v, "eigen-1").farbe === "rot", "Absage");
pruefe(vorigesErgebnis(v, "kat-ohne-antrag").label === "in Bearbeitung", "gemerkt, aber Antrag nie geöffnet");
pruefe(vorigesErgebnis(v, "kat-neu").label === "nicht beantragt" && !vorigesErgebnis(v, "kat-neu").beantragt, "neu gemerkt: nicht beantragt");
pruefe(vorigesErgebnis(v, "nicht-gemerkt").label === "nicht beantragt", "wieder entfernte Förderung: nicht beantragt");
const frei = { merkliste: ["x"], antraege: { x: { status: "anderer", statusFrei: "Teilzusage" } } };
pruefe(vorigesErgebnis(frei, "x").label === "Teilzusage", "freier Status wird angezeigt");

if (fehler) {
  console.error(`\n✗ ${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\n✓ Ausgaben OK: Die neue Ausgabe übernimmt nur, was sie darf.");
