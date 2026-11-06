// ============================================================
// Teilantrag-Test: prüft, was ein Antrag über einzelne Bausteine kostet und
// beantragt – ohne die App zu starten. Wichtigster Punkt: Die beantragte
// Summe rechnet nur mit den abgedeckten Posten, nie mit den Gesamtkosten.
//
// Ausfuehren:  npm run teilantrag   (oder: node tools/teilantrag-test.mjs)
// ============================================================

import { GEMEINSAM, bausteinEntfernen } from "../src/lib/bausteine.js";
import {
  abdeckungVon,
  decktPosten,
  abdeckungText,
  teilKosten,
  teilKostenSumme,
  istAufgeteilt,
  aufteilungRest,
  teilFinanzBetrag,
  teilAntragHinweise,
  teilBeantragt,
  teilAntragAbschnitte,
  abdeckungAufraeumen,
  hatTeilAntraege,
} from "../src/lib/teilantrag.js";
import { ausgabeBefuellen } from "../src/lib/ausgaben.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

const bausteine = [
  { id: "m", name: "Konzert März" },
  { id: "j", name: "Konzert Juni" },
];
// Gesamtkosten 3.750 €: März 1.000, Juni 2.050, gemeinsam 700.
const kfp = () => ({
  kosten: [
    {
      name: "Honorare",
      posten: [
        { id: "k1", bezeichnung: "Gagen März", betrag: "1000", bausteinId: "m" },
        { id: "k2", bezeichnung: "Gagen Juni", betrag: "4 × 500", bausteinId: "j" },
        { id: "k3", bezeichnung: "Moderation", betrag: "300", bausteinId: "" },
      ],
    },
    {
      name: "Werbung",
      posten: [
        { id: "k4", bezeichnung: "Plakate", betrag: "400" },
        { id: "k5", bezeichnung: "Flyer Juni", betrag: "50", bausteinId: "j" },
      ],
    },
  ],
  finanzierung: [
    {
      name: "",
      posten: [
        { foerderId: "stadt", bezeichnung: "", betrag: "1500", anteile: { m: "1000", [GEMEINSAM]: "500" } },
        { foerderId: "fonds", bezeichnung: "", betrag: "2000", anteile: { j: "2000" } },
        { foerderId: "", bezeichnung: "Eigenmittel", betrag: "250" },
      ],
    },
  ],
});
const nummern = (liste) => liste.flatMap((k) => k.posten.map((e) => e.nummer)).join(" ");
const name = (p) => p.bezeichnung || { stadt: "Kulturamt", fonds: "Musikfonds" }[p.foerderId];
const zellen = (abschnitte) => abschnitte.flatMap((a) => [...a.absaetze, ...a.tabelle.flat()]).join(" | ");

console.log("Abdeckung:");
pruefe(abdeckungVon({ abdeckung: ["m"] }, []) === null, "ohne Bausteine: ganzes Projekt");
pruefe(abdeckungVon({}, bausteine) === null, "ohne Angabe: ganzes Projekt (wie bisher)");
pruefe(abdeckungVon({ abdeckung: ["m", "weg", GEMEINSAM] }, bausteine).join() === `m,${GEMEINSAM}`, "Verweise auf fehlende Bausteine fallen weg");
pruefe(abdeckungVon({ abdeckung: [] }, bausteine).length === 0, "leere Auswahl bleibt leer, wird nicht zum ganzen Projekt");
pruefe(kfp().kosten.flatMap((k) => k.posten).every((p) => decktPosten(null, p, bausteine)), "ganzes Projekt deckt jeden Posten");
pruefe(abdeckungText(null, bausteine) === "das ganze Projekt", "Text: ganzes Projekt");
pruefe(abdeckungText(["m"], bausteine) === "Konzert März", "Text: ein Baustein");
pruefe(abdeckungText(["j", "m", GEMEINSAM], bausteine) === "Konzert März, Konzert Juni und gemeinsame Kosten", "Text: mehrere Teile");
pruefe(abdeckungText([], bausteine) === "noch keinen Teil", "Text: nichts gewählt");

console.log("Kosten eines Teils:");
pruefe(nummern(teilKosten(kfp().kosten, ["m"], bausteine)) === "1.1", "März: nur sein Posten");
pruefe(nummern(teilKosten(kfp().kosten, ["j"], bausteine)) === "1.2 2.2", "Juni: Nummern wie im Gesamtplan");
pruefe(teilKostenSumme(kfp().kosten, ["j"], bausteine) === 2050, "Juni: Summe mit Formel-Betrag");
pruefe(nummern(teilKosten(kfp().kosten, ["m", GEMEINSAM], bausteine)) === "1.1 1.3 2.1", "März plus gemeinsame Kosten");
pruefe(teilKostenSumme(kfp().kosten, ["m", GEMEINSAM], bausteine) === 1700, "März plus gemeinsam: 1.700 €");
pruefe(teilKosten(kfp().kosten, ["m"], bausteine).map((k) => k.nummer).join() === "1", "Kategorien ohne Treffer fehlen");
pruefe(teilKostenSumme(kfp().kosten, [], bausteine) === 0, "nichts gewählt: keine Kosten");

console.log("Aufteilung der Finanzierung:");
const [stadt, fonds, eigen] = kfp().finanzierung[0].posten;
pruefe(istAufgeteilt(stadt, bausteine) && !istAufgeteilt(eigen, bausteine), "aufgeteilt erkannt");
pruefe(!istAufgeteilt(stadt, []), "ohne Bausteine ist nichts aufgeteilt");
pruefe(!istAufgeteilt({ betrag: "100", anteile: { m: "  " } }, bausteine), "leere Anteile zählen nicht");
pruefe(aufteilungRest(stadt, bausteine) === 0, "genau verteilt: Rest 0");
pruefe(aufteilungRest({ betrag: "100", anteile: { m: "2 × 15" } }, bausteine) === 70, "Rest mit Formel im Anteil");
pruefe(aufteilungRest({ betrag: "100", anteile: { m: "150" } }, bausteine) === -50, "zu viel verteilt: negativer Rest");
pruefe(aufteilungRest({ betrag: "100", anteile: { weg: "100" } }, bausteine) === 100, "Anteil eines fehlenden Bausteins zählt nicht");
pruefe(teilFinanzBetrag(stadt, ["m"], bausteine) === 1000, "Anteil für März");
pruefe(teilFinanzBetrag(stadt, ["m", GEMEINSAM], bausteine) === 1500, "Anteil für März plus gemeinsam");
pruefe(teilFinanzBetrag(eigen, ["m"], bausteine) === null, "nicht aufgeteilt: kein Teilbetrag");

console.log("Beantragte Summe:");
pruefe(teilBeantragt(kfp(), "stadt", ["m", GEMEINSAM], bausteine) === 1500, "eingeplanter Förderer: sein Anteil an den Teilen");
pruefe(teilBeantragt(kfp(), "stadt", ["m"], bausteine) === 1000, "nur März: nur der März-Anteil");
pruefe(teilBeantragt(kfp(), "fonds", ["j"], bausteine) === 2000, "Fonds für Juni");
const ungeteilt = kfp();
delete ungeteilt.finanzierung[0].posten[1].anteile;
pruefe(teilBeantragt(ungeteilt, "fonds", ["j"], bausteine) === 2000, "nicht aufgeteilter Förderer: sein ganzer Betrag");
pruefe(teilBeantragt(kfp(), "neu", ["j"], bausteine) === 50, "nicht eingeplant: Juni-Kosten minus Juni-Anteile anderer");
pruefe(teilBeantragt(kfp(), "neu", ["m"], bausteine) === 0, "nicht eingeplant und gedeckt: 0");
const ohneFinanz = kfp();
ohneFinanz.finanzierung = [];
pruefe(teilBeantragt(ohneFinanz, "neu", ["j"], bausteine) === 2050, "ohne Finanzierung: genau die Teil-Kosten, nie 3.750");

console.log("Hinweise:");
const h = teilAntragHinweise(kfp(), ["j"], bausteine);
pruefe(!h.nichtsAbgedeckt && h.nichtAufgeteilt.length === 1 && h.nichtAufgeteilt[0].bezeichnung === "Eigenmittel", "nicht aufgeteilte Quelle wird gemeldet");
pruefe(teilAntragHinweise(kfp(), [], bausteine).nichtsAbgedeckt, "leere Auswahl wird gemeldet");
pruefe(teilAntragHinweise(kfp(), null, bausteine).nichtAufgeteilt.length === 0, "ganzes Projekt: keine Hinweise");

console.log("PDF-Abschnitte:");
const juni = teilAntragAbschnitte(kfp(), "fonds", "Musikfonds", ["j"], bausteine, name);
const text = zellen(juni);
pruefe(juni.length === 3, "drei Abschnitte wie beim ganzen Projekt");
pruefe(text.includes("Dieser Antrag umfasst Konzert Juni."), "nennt den Teil");
pruefe(text.includes("1.2  Gagen Juni") && text.includes("2.2  Flyer Juni"), "Posten mit Original-Nummern");
pruefe(!text.includes("Gagen März") && !text.includes("Plakate"), "fremde und gemeinsame Posten fehlen");
pruefe(text.includes("**Kosten dieses Teils") && text.includes("**2.050,00 €"), "Summe dieses Teils");
pruefe(!text.includes("Kulturamt |"), "Quelle ohne Anteil an Juni steht nicht in der Tabelle");
pruefe(text.includes("Außerdem für das ganze Projekt") && text.includes("Eigenmittel 250,00 €"), "nicht aufgeteilte Quelle steht gesondert");
pruefe(juni[2].tabelle[1].join() === "**Musikfonds (Fehlbetrag),**2.000,00 €", "beantragte Summe");
const maerz = zellen(teilAntragAbschnitte(kfp(), "fonds", "Musikfonds", ["m", GEMEINSAM], bausteine, name));
pruefe(maerz.includes("Kulturamt | 1.500,00 €") && maerz.includes("**Summe andere Mittel"), "aufgeteilte Quelle mit ihrem Anteil");
pruefe(zellen(teilAntragAbschnitte(kfp(), "x", "", [], bausteine, name)).includes("Diesem Teil sind keine Kosten zugeordnet."), "leere Auswahl: verständlicher Hinweis");

console.log("Aufräumen:");
const p1 = { bausteine: structuredClone(bausteine), kfp: kfp(), antraege: { stadt: { abdeckung: ["m", "j"] }, fonds: { abdeckung: null }, alt: {} } };
bausteinEntfernen(p1.bausteine, p1.kfp, "j");
pruefe(!("j" in p1.kfp.finanzierung[0].posten[1].anteile), "Baustein entfernen löscht seine Anteile");
pruefe(abdeckungAufraeumen(p1) === true, "meldet eine Änderung");
pruefe(p1.antraege.stadt.abdeckung.join() === "m", "Abdeckung verliert den entfernten Baustein");
pruefe(!("anteile" in p1.kfp.finanzierung[0].posten[1]), "leere Anteile fallen ganz weg");
pruefe(p1.antraege.fonds.abdeckung === null && !("abdeckung" in p1.antraege.alt), "ganze Projekte bleiben unberührt");
pruefe(abdeckungAufraeumen(p1) === false, "zweiter Lauf ändert nichts");
const p2 = { bausteine: [], kfp: kfp(), antraege: { stadt: { abdeckung: ["m"] } } };
abdeckungAufraeumen(p2);
pruefe(p2.antraege.stadt.abdeckung === null, "ohne Bausteine wieder ganzes Projekt");
pruefe(p2.kfp.finanzierung[0].posten.every((p) => !("anteile" in p)), "ohne Bausteine keine Anteile");
const p3 = { bausteine: structuredClone(bausteine), kfp: kfp(), antraege: { stadt: { abdeckung: ["j"] } } };
p3.bausteine = p3.bausteine.filter((b) => b.id !== "j");
abdeckungAufraeumen(p3);
pruefe(Array.isArray(p3.antraege.stadt.abdeckung) && p3.antraege.stadt.abdeckung.length === 0, "letzter Teil entfernt: bleibt leer statt ganzes Projekt");

console.log("Nächste Ausgabe:");
const vorige = {
  id: "alt",
  bausteine: structuredClone(bausteine),
  formular: { zeitraum: "" },
  kfp: kfp(),
  merkliste: ["stadt"],
  antraege: { stadt: { status: "zusage", abdeckung: ["m"], kontakt: {}, checkliste: [] } },
};
const frisch = () => ({ id: "neu", bausteine: [], formular: {}, kfp: { kosten: [], finanzierung: [] }, merkliste: [], antraege: {} });
const mitAllem = ausgabeBefuellen(frisch(), vorige, { texte: true, kostenplan: true, betraege: true, merkliste: true }, () => []);
pruefe(mitAllem.antraege.stadt.abdeckung.join() === "m", "Abdeckung kommt mit den Bausteinen mit");
pruefe(mitAllem.kfp.finanzierung[0].posten[0].anteile.m === "1000", "Anteile kommen mit den Beträgen mit");
const ohneBetraege = ausgabeBefuellen(frisch(), vorige, { texte: false, kostenplan: true, betraege: false, merkliste: true }, () => []);
pruefe(ohneBetraege.kfp.finanzierung[0].posten.every((p) => !("anteile" in p)), "ohne Beträge auch keine Anteile");
const ohneBausteine = ausgabeBefuellen(frisch(), vorige, { texte: false, kostenplan: false, betraege: false, merkliste: true }, () => []);
pruefe(ohneBausteine.antraege.stadt.abdeckung === null, "ohne Bausteine gilt der Antrag fürs ganze Projekt");

console.log("Gibt es Teil-Anträge?");
const teilProjekt = (abdeckung, merkliste = ["f"]) => ({
  bausteine: [{ id: "m", name: "März" }],
  merkliste,
  antraege: { f: { abdeckung } },
});
pruefe(hatTeilAntraege(teilProjekt(["m"])) === true, "Antrag über einen Teil");
pruefe(hatTeilAntraege(teilProjekt([])) === true, "leere Abdeckung zählt auch (Warnung im Antrag)");
pruefe(hatTeilAntraege(teilProjekt(null)) === false, "nur Anträge über das ganze Projekt");
pruefe(hatTeilAntraege(teilProjekt(["m"], [])) === false, "Förderung nicht mehr gemerkt");
pruefe(hatTeilAntraege({ ...teilProjekt(["m"]), bausteine: [] }) === false, "ohne Bausteine nie");
pruefe(hatTeilAntraege({}) === false, "leeres Projekt");

if (fehler) {
  console.error(`\n✗ ${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\n✓ Teilantrag OK: beantragt wird nur, was der Antrag abdeckt.");
