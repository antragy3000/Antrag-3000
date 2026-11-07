// ============================================================
// Gruppen-Übersicht-Test: prüft die Zahlen je Ausgabe (B) und den
// Förderer-Vergleich über die Jahre (A) – ohne die App zu starten.
//
// Ausfuehren:  npm run gruppenuebersicht   (oder: node tools/gruppenuebersicht-test.mjs)
// ============================================================

import {
  ausgabenReihenfolge,
  antragBeantragt,
  ausgabeZahlen,
  foerdererVergleich,
  gruppenUebersicht,
} from "../src/lib/gruppenUebersicht.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

const posten = (id, betrag, extra = {}) => ({ id, bezeichnung: id, erlaeuterung: "", betrag: String(betrag), ...extra });
const heute = new Date(2027, 2, 1); // 1. März 2027

// Ausgabe 2026: Kulturamt (Zusage), Musikfonds (Zusage, Soll in der
// Abrechnung kleiner), Land (Absage). Kosten 18.400.
const p26 = {
  id: "p26", name: "Klangraum 2026", gruppeId: "g", vorgaengerId: "p25",
  merkliste: ["amt", "fonds", "land"],
  antraege: { amt: { status: "zusage" }, fonds: { status: "zusage" }, land: { status: "absage" } },
  kfp: {
    kosten: [{ name: "Honorare", posten: [posten("k1", 12000), posten("k2", 6400)] }],
    finanzierung: [{ name: "Förderung", posten: [posten("f1", 4000, { foerderId: "amt" }), posten("f2", 7500, { foerderId: "fonds" }), posten("f3", 2500, { foerderId: "land" }), posten("e", 4400)] }],
  },
  abrechnung: {
    quellen: [
      { id: "q1", typ: "foerderung", foerderId: "amt", soll: "4000", nachweisFrist: "2027-06-30" },
      { id: "q2", typ: "foerderung", foerderId: "fonds", soll: "7000", nachweisFrist: "2026-12-31" },
      { id: "q3", typ: "eigenmittel", foerderId: "", soll: "4400", nachweisFrist: "2027-04-15" },
    ],
    belege: [
      { id: "b1", brutto: "5000", zuordnungen: [{ quelleId: "q1", betrag: "3000" }, { quelleId: "q3", betrag: "2000" }] },
      { id: "b2", brutto: "6800", zuordnungen: [{ quelleId: "q2", betrag: "6800" }] },
    ],
  },
};
const p25 = {
  id: "p25", name: "Klangraum 2025", gruppeId: "g", archiviert: true,
  merkliste: ["amt"], antraege: { amt: { status: "zusage" } },
  kfp: { kosten: [{ name: "K", posten: [posten("k", 5000)] }], finanzierung: [] },
};
// Ausgabe 2027 mit Teilen: Stiftung nur für März (Teil-Antrag).
const p27 = {
  id: "p27", name: "Klangraum 2027", gruppeId: "g", vorgaengerId: "p26",
  bausteine: [{ id: "m", name: "März" }, { id: "j", name: "Juni" }],
  merkliste: ["stift", "amt"],
  antraege: { stift: { status: "in_bearbeitung", abdeckung: ["m"] } },
  kfp: {
    kosten: [{ name: "Honorare", posten: [posten("k1", 3000, { bausteinId: "m" }), posten("k2", 5000, { bausteinId: "j" })] }],
    finanzierung: [{ name: "F", posten: [posten("f1", 1000, { foerderId: "amt" })] }],
  },
};
const fremd = { id: "x", name: "Workshop", gruppeId: "", merkliste: ["amt"], kfp: { kosten: [], finanzierung: [] } };

console.log("Reihenfolge der Ausgaben");
pruefe(ausgabenReihenfolge([p27, p25, p26]).map((p) => p.id).join() === "p25,p26,p27", "Kette über vorgaengerId: alt nach neu");
pruefe(ausgabenReihenfolge([{ id: "a" }, { id: "b" }]).map((p) => p.id).join() === "a,b", "ohne Kette: Reihenfolge der Liste");
const ring = ausgabenReihenfolge([{ id: "a", vorgaengerId: "b" }, { id: "b", vorgaengerId: "a" }]);
pruefe(ring.length === 2, "Ring in der Kette hängt nicht");
pruefe(ausgabenReihenfolge([{ id: "a", vorgaengerId: "geloescht" }]).length === 1, "Vorgänger gelöscht: Ausgabe bleibt");

console.log("Beantragte Summe je Antrag");
pruefe(antragBeantragt(p26.kfp, "fonds", p26.antraege.fonds, []) === 7500, "ganzes Projekt: eingeplanter Betrag");
const ohneEinplanung = { kosten: [{ name: "K", posten: [posten("k", 10000)] }], finanzierung: [{ name: "F", posten: [posten("e", 2500)] }] };
pruefe(antragBeantragt(ohneEinplanung, "neu", undefined, []) === 7500, "nicht eingeplant: Kosten minus übrige Mittel");
pruefe(antragBeantragt(p27.kfp, "stift", p27.antraege.stift, p27.bausteine) === 3000, "Teil-Antrag: nur Kosten des abgedeckten Teils");

console.log("Zahlen je Ausgabe (B)");
const z26 = ausgabeZahlen(p26, heute);
pruefe(z26.kosten === 18400, "Kosten = Summe des Kostenplans");
pruefe(z26.beantragt === 14000, "Beantragt: alle Anträge der Merkliste, auch die Absage");
pruefe(z26.bewilligt === 11000, "Bewilligt: nur Zusagen, Soll der Geldquelle hat Vorrang (4.000 + 7.000)");
pruefe(z26.abgerechnet === 9800, "Abgerechnet: nur Zuordnungen auf Förderer-Quellen");
pruefe(z26.nachweis.offen === 2 && z26.nachweis.naechste === "2027-04-15", "Nachweis: zwei Fristen ab heute, nächste zuerst");
const z25 = ausgabeZahlen(p25, heute);
pruefe(z25.bewilligt === 5000 && z25.abgerechnet === 0 && z25.nachweis.naechste === null, "ohne Abrechnung: Bewilligt aus dem Antrag, kein Nachweis");
const zLeer = ausgabeZahlen({ id: "leer" }, heute);
pruefe(zLeer.kosten === 0 && zLeer.beantragt === 0 && zLeer.nachweis.offen === 0, "leeres Projekt: alles 0");
pruefe(ausgabeZahlen(p27, heute).beantragt === 4000, "Ausgabe mit Teilen: 3.000 (Teil) + 1.000 (eingeplant)");

console.log("Förderer im Vergleich (A)");
const namen = { amt: "Kulturamt", fonds: "Musikfonds", land: "Land", stift: "Stiftung" };
const v = foerdererVergleich([p25, p26, p27], (id) => namen[id]);
pruefe(v.map((r) => r.id).join() === "stift,amt,fonds,land", "Zeilen: zuerst die der neuesten Ausgabe");
const amt = v.find((r) => r.id === "amt");
pruefe(amt.zellen.length === 3 && amt.zellen[0].label === "Zusage erhalten" && amt.zellen[0].betrag === 5000, "Zelle: Status und Betrag der Ausgabe");
pruefe(amt.zellen[2].label === "in Bearbeitung" && amt.zellen[2].betrag === 1000, "noch nicht geöffneter Antrag: Standard-Status");
const land = v.find((r) => r.id === "land");
pruefe(!land.zellen[0].beantragt && land.zellen[0].label === "nicht beantragt" && land.zellen[1].farbe === "rot", "nicht auf der Merkliste: „nicht beantragt“");

console.log("Ganze Gruppe");
const u = gruppenUebersicht({ id: "g", name: "Klangraum Festival" }, [p27, fremd, p25, p26], { nameVon: (id) => namen[id], heute });
pruefe(u.ausgaben.map((p) => p.id).join() === "p25,p26,p27", "nur Projekte der Gruppe, auch archivierte");
pruefe(u.summe.kosten === 5000 + 18400 + 8000 && u.summe.beantragt === 5000 + 14000 + 4000, "Summen gehen mit den Ausgaben auf");
pruefe(gruppenUebersicht({ id: "leer" }, [p25], { nameVon: () => "" }) === null, "Gruppe ohne Ausgaben: nichts");
pruefe(gruppenUebersicht(null, [p25], { nameVon: () => "" }) === null, "keine Gruppe: nichts");
pruefe(JSON.stringify(p26).includes("summe") === false, "Ansicht ändert die Projekte nicht");

if (fehler) {
  console.error(`\n${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\nAlle Prüfungen bestanden.");
