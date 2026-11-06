// ============================================================
// Bausteine-Test: prüft Filter, Summen und Nummern im Kostenplan mit
// Bausteinen – ohne die App zu starten. Wichtigster Punkt: Filtern darf
// nie eine Nummer verschieben, weil Belege daran hängen.
//
// Ausfuehren:  npm run bausteine   (oder: node tools/bausteine-test.mjs)
// ============================================================

import {
  ALLE,
  GEMEINSAM,
  bausteinVon,
  bausteinName,
  kostenGefiltert,
  summeGefiltert,
  bausteinNamePruefen,
  bausteinEntfernen,
  bausteineUmstellen,
  bausteinTexteUebernehmen,
  bausteinAbschnitte,
} from "../src/lib/bausteine.js";
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
  { id: "maerz", name: "Konzert März" },
  { id: "mai", name: "Konzert Mai" },
];
const kosten = [
  {
    name: "Honorare",
    posten: [
      { id: "k1", bezeichnung: "Gagen März", betrag: 1000, bausteinId: "maerz" },
      { id: "k2", bezeichnung: "Gagen Mai", betrag: 2000, bausteinId: "mai" },
      { id: "k3", bezeichnung: "Moderation", betrag: 300, bausteinId: "" },
    ],
  },
  {
    name: "Werbung",
    posten: [
      { id: "k4", bezeichnung: "Plakate", betrag: 400 },
      { id: "k5", bezeichnung: "Flyer Mai", betrag: 50, bausteinId: "mai" },
    ],
  },
  { name: "Leer", posten: [] },
];
const betrag = (p) => p.betrag;
const nummern = (liste) => liste.flatMap((k) => k.posten.map((e) => e.nummer)).join(" ");

console.log("Zuordnung:");
pruefe(bausteinVon(kosten[0].posten[0], bausteine) === "maerz", "Posten mit Baustein");
pruefe(bausteinVon(kosten[1].posten[0], bausteine) === "", "Posten ohne Feld ist gemeinsam");
pruefe(bausteinVon({ bausteinId: "weg" }, bausteine) === "", "Verweis auf fehlenden Baustein zählt als gemeinsam");
pruefe(bausteinVon({ bausteinId: "maerz" }, []) === "", "ohne Bausteine ist alles gemeinsam");
pruefe(bausteinName(kosten[0].posten[1], bausteine) === "Konzert Mai", "Name des Bausteins");
pruefe(bausteinName({ bausteinId: "weg" }, bausteine) === "gemeinsam", "Name bei gemeinsamem Posten");

console.log("Filter und Nummern:");
const alle = kostenGefiltert(kosten, ALLE, bausteine);
pruefe(alle.length === 3, "ALLE zeigt jede Kategorie, auch leere");
pruefe(nummern(alle) === "1.1 1.2 1.3 2.1 2.2", "ALLE: Nummern wie im Plan");
const mai = kostenGefiltert(kosten, "mai", bausteine);
pruefe(nummern(mai) === "1.2 2.2", "Filter: Nummern bleiben die originalen");
pruefe(mai.map((k) => k.nummer).join() === "1,2", "Filter: Kategorien behalten ihre Nummer");
pruefe(mai[1].posten[0].index === 1, "Filter: Position im Original bleibt bekannt");
const maerz = kostenGefiltert(kosten, "maerz", bausteine);
pruefe(maerz.length === 1 && nummern(maerz) === "1.1", "Kategorien ohne Treffer fehlen");
const gemeinsam = kostenGefiltert(kosten, GEMEINSAM, bausteine);
pruefe(nummern(gemeinsam) === "1.3 2.1", "gemeinsam: leeres Feld und fehlendes Feld");
pruefe(kostenGefiltert(kosten, "unbekannt", bausteine).length === 0, "unbekannter Filter zeigt nichts");
pruefe(kostenGefiltert([], ALLE, bausteine).length === 0, "leerer Plan bleibt leer");

console.log("Summen:");
pruefe(summeGefiltert(kosten, ALLE, bausteine, betrag) === 3750, "alle Posten");
pruefe(summeGefiltert(kosten, "mai", bausteine, betrag) === 2050, "ein Baustein");
pruefe(summeGefiltert(kosten, GEMEINSAM, bausteine, betrag) === 700, "gemeinsame Posten");
const teile = summeGefiltert(kosten, "maerz", bausteine, betrag) + summeGefiltert(kosten, "mai", bausteine, betrag) + summeGefiltert(kosten, GEMEINSAM, bausteine, betrag);
pruefe(teile === 3750, "Bausteine plus gemeinsam ergeben genau die Gesamtsumme");

console.log("Namen:");
pruefe(bausteinNamePruefen("Konzert Juni", bausteine).ok, "freier Name ist erlaubt");
pruefe(bausteinNamePruefen(" Konzert Juni ", bausteine).name === "Konzert Juni", "Name wird gesäubert");
const doppelt = bausteinNamePruefen("konzert mai", bausteine);
pruefe(!doppelt.ok && doppelt.fehler.includes("Konzert Mai"), "doppelter Name wird mit Meldung abgelehnt");
pruefe(bausteinNamePruefen("KONZERT MAI", bausteine, "mai").ok, "eigener Baustein darf die Schreibweise ändern");
const leer = bausteinNamePruefen("  ", bausteine);
pruefe(!leer.ok && leer.fehler === "", "leere Eingabe: nicht erlaubt, ohne Meldung");

console.log("Entfernen:");
const b2 = structuredClone(bausteine);
const kfp2 = { kosten: structuredClone(kosten), finanzierung: [] };
bausteinEntfernen(b2, kfp2, "mai");
pruefe(b2.map((b) => b.id).join() === "maerz", "nur dieser Baustein ist weg");
pruefe(kfp2.kosten.flatMap((k) => k.posten).length === 5, "alle Posten bleiben erhalten");
pruefe(kfp2.kosten[0].posten[1].bausteinId === "" && kfp2.kosten[1].posten[1].bausteinId === "", "seine Posten sind jetzt gemeinsam");
pruefe(kfp2.kosten[0].posten[0].bausteinId === "maerz", "andere Zuordnungen bleiben");
pruefe(nummern(kostenGefiltert(kfp2.kosten, ALLE, b2)) === "1.1 1.2 1.3 2.1 2.2", "Nummern bleiben gleich");

console.log("Ältere Stände:");
const alt = { kfp: { kosten: [{ name: "A", posten: [{ id: "x", bausteinId: "weg" }, { id: "y" }] }], finanzierung: [] } };
pruefe(bausteineUmstellen(alt) === true, "meldet eine Änderung");
pruefe(Array.isArray(alt.bausteine) && alt.bausteine.length === 0, "bekommt eine leere Liste");
pruefe(alt.kfp.kosten[0].posten[0].bausteinId === "", "Verweis auf fehlenden Baustein wird gemeinsam");
pruefe(!("bausteinId" in alt.kfp.kosten[0].posten[1]), "Posten ohne Feld bleibt unberührt");
pruefe(bausteineUmstellen(alt) === false, "zweites Öffnen ändert nichts");

console.log("Nächste Ausgabe:");
const quelle = {
  id: "alt",
  bausteine: structuredClone(bausteine),
  formular: { zeitraum: "" },
  kfp: { kosten: structuredClone(kosten), finanzierung: [] },
  merkliste: [],
};
const frisch = () => ({ id: "neu", bausteine: [], formular: {}, kfp: { kosten: [], finanzierung: [] }, merkliste: [], antraege: {} });
const mitPlan = ausgabeBefuellen(frisch(), quelle, { texte: false, kostenplan: true, betraege: false, merkliste: false }, () => []);
pruefe(mitPlan.bausteine.map((b) => b.name).join() === "Konzert März,Konzert Mai", "mit Kostenplan kommen die Bausteine mit");
pruefe(mitPlan.kfp.kosten[0].posten[1].bausteinId === "mai", "Posten bleiben ihrem Baustein zugeordnet");
mitPlan.bausteine[0].name = "geändert";
pruefe(quelle.bausteine[0].name === "Konzert März", "Änderung wirkt nicht auf die vorige Ausgabe");
const mitTexten = ausgabeBefuellen(frisch(), quelle, { texte: true, kostenplan: false, betraege: false, merkliste: false }, () => []);
pruefe(mitTexten.bausteine.length === 2, "mit Texten kommen die Bausteine auch mit");
const ohne = ausgabeBefuellen(frisch(), quelle, { texte: false, kostenplan: false, betraege: false, merkliste: false }, () => []);
pruefe(ohne.bausteine.length === 0, "ohne Texte und Kostenplan bleiben sie weg");

console.log("Texte je Baustein:");
const mitTexten3 = [
  { id: "maerz", name: "Konzert März", beschreibung: "alt" },
  { id: "mai", name: "Konzert Mai" },
];
bausteinTexteUebernehmen(mitTexten3, {
  maerz: { beschreibung: "  Kammermusik im Hof  ", zeitraum: "14.03.2027", ort: "" },
  weg: { beschreibung: "gehört zu keinem Baustein" },
});
pruefe(mitTexten3[0].beschreibung === "Kammermusik im Hof" && mitTexten3[0].zeitraum === "14.03.2027", "Texte werden gesäubert übernommen");
pruefe(!("ort" in mitTexten3[0]), "leeres Feld fällt weg");
pruefe(mitTexten3[0].name === "Konzert März" && mitTexten3[0].id === "maerz", "Name und Kennung bleiben");
pruefe(!("beschreibung" in mitTexten3[1]) && mitTexten3.length === 2, "Texte gelöschter Bausteine fallen weg");
const ab = bausteinAbschnitte(mitTexten3, null);
pruefe(ab.length === 1 && ab[0].ueberschrift === "Projektteil: Konzert März", "ganzes Projekt: alle Bausteine mit Text");
pruefe(ab[0].absaetze.join(" | ") === "Kammermusik im Hof | Zeitraum: 14.03.2027", "Beschreibung, dann Zeitraum");
pruefe(bausteinAbschnitte(mitTexten3, ["mai"]).length === 0, "Teil-Antrag: nur abgedeckte Bausteine");
pruefe(bausteinAbschnitte(mitTexten3, ["maerz", GEMEINSAM]).length === 1, "Teil-Antrag mit gemeinsamen Kosten");
pruefe(bausteinAbschnitte(bausteine, null).length === 0, "ohne Texte keine Abschnitte (PDF wie bisher)");
const quelleTexte = { ...quelle, bausteine: [{ id: "maerz", name: "Konzert März", beschreibung: "Hof", zeitraum: "März 2026", ort: "Musterstadt" }] };
const nurPlan = ausgabeBefuellen(frisch(), quelleTexte, { texte: false, kostenplan: true, betraege: false, merkliste: false }, () => []);
pruefe(nurPlan.bausteine[0].name === "Konzert März" && !("beschreibung" in nurPlan.bausteine[0]) && !("ort" in nurPlan.bausteine[0]), "nur Kostenplan: Bausteine ohne Texte");
const mitText = ausgabeBefuellen(frisch(), quelleTexte, { texte: true, kostenplan: false, betraege: false, merkliste: false }, () => []);
pruefe(mitText.bausteine[0].beschreibung === "Hof" && mitText.bausteine[0].ort === "Musterstadt", "mit Texten: Beschreibung und Ort kommen mit");
pruefe(!("zeitraum" in mitText.bausteine[0]), "Zeitraum der vorigen Ausgabe kommt nie mit");
pruefe(quelleTexte.bausteine[0].zeitraum === "März 2026", "vorige Ausgabe bleibt unverändert");

if (fehler) {
  console.error(`\n✗ ${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\n✓ Bausteine OK: Filtern ändert nie eine Nummer.");
