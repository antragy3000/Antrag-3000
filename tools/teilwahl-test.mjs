// ============================================================
// Teil-Wahl-Test: prüft, welcher Teil im Projektmenü als gewählt gilt, wie
// ein Teil direkt im Menü angelegt wird und was Merkliste, Belege und
// Kostenstellen beim Blick auf einen Teil zeigen – ohne die App zu starten.
//
// Ausfuehren:  npm run teilwahl   (oder: node tools/teilwahl-test.mjs)
// ============================================================

import {
  teilImBlick,
  teilAnlegen,
  neueTeilId,
  decktTeil,
  merklisteFuerTeil,
  belegeFuerTeil,
  kostenstellenVonTeil,
} from "../src/lib/teilwahl.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

const projekt = () => ({
  id: "p27",
  name: "Klangraum 2027",
  bausteine: [
    { id: "m", name: "Konzert März" },
    { id: "j", name: "Konzert Juni" },
  ],
});

console.log("Welcher Teil ist im Blick?");
const p = projekt();
pruefe(teilImBlick({ projektId: "p27", teilId: "m" }, p)?.name === "Konzert März", "gewählter Teil wird gefunden");
pruefe(teilImBlick(null, p) === null, "ohne Wahl: ganze Ausgabe");
pruefe(teilImBlick({ projektId: "p26", teilId: "m" }, p) === null, "Wahl aus einer anderen Ausgabe (gleiche Kennung) gilt nicht");
pruefe(teilImBlick({ projektId: "p27", teilId: "weg" }, p) === null, "gelöschter Teil: ganze Ausgabe");
pruefe(teilImBlick({ projektId: "p27", teilId: "m" }, { ...p, archiviert: true }) === null, "archivierte Ausgabe: keine Teil-Wahl");
pruefe(teilImBlick({ projektId: "p27", teilId: "m" }, { id: "p27" }) === null, "Projekt ohne Teile");
pruefe(teilImBlick({ projektId: "p27", teilId: "m" }, null) === null, "kein Projekt");

console.log("Teil im Menü anlegen");
let zaehler = 0;
const id = () => `neu-${++zaehler}`;
const q = projekt();
const r1 = teilAnlegen(q, "  Konzert Mai  ", id);
pruefe(r1.ok && r1.teil.name === "Konzert Mai" && r1.teil.id === "neu-1", "Name wird getrimmt, Kennung vergeben");
pruefe(q.bausteine.length === 3 && q.bausteine[2] === r1.teil, "Teil steht am Ende der Liste");
const r2 = teilAnlegen(q, "konzert märz", id);
pruefe(!r2.ok && r2.fehler.includes("Konzert März") && q.bausteine.length === 3, "doppelter Name (Groß/klein egal) wird abgelehnt");
const r3 = teilAnlegen(q, "   ", id);
pruefe(!r3.ok && r3.fehler === "" && q.bausteine.length === 3, "leerer Name: nichts angelegt, keine Meldung");
const ohne = { id: "solo", name: "Workshop" };
const r4 = teilAnlegen(ohne, "Tag 1", id);
pruefe(r4.ok && Array.isArray(ohne.bausteine) && ohne.bausteine.length === 1, "Projekt ohne Teile bekommt die Liste neu");
const a = neueTeilId(), b = neueTeilId();
pruefe(typeof a === "string" && a.length > 8 && a !== b, "Kennungen sind eindeutig");

console.log("Merkliste im Teil");
const teile = [{ id: "m", name: "März" }, { id: "j", name: "Juni" }];
const maerz = teile[0];
const antraege = { amt: { abdeckung: ["j"] }, fonds: { abdeckung: null }, stift: { abdeckung: ["m", "gemeinsam"] }, leer: { abdeckung: [] } };
pruefe(decktTeil(antraege.fonds, "m", teile), "Antrag über das ganze Projekt deckt jeden Teil");
pruefe(decktTeil(undefined, "m", teile), "noch kein Antrag angelegt: zählt als ganzes Projekt");
pruefe(!decktTeil(antraege.amt, "m", teile), "Antrag nur über Juni deckt März nicht");
pruefe(decktTeil(antraege.stift, "m", teile), "Antrag über März und gemeinsam deckt März");
pruefe(!decktTeil(antraege.leer, "m", teile), "leere Abdeckung deckt nichts");
const ml = merklisteFuerTeil(["amt", "fonds", "stift", "neu", "leer"], antraege, maerz, teile);
pruefe(ml.ids.join() === "fonds,stift,neu" && ml.ausgeblendet === 2, "Merkliste: Reihenfolge bleibt, zwei ausgeblendet");
const alle = merklisteFuerTeil(["amt", "fonds"], antraege, null, teile);
pruefe(alle.ids.length === 2 && alle.ausgeblendet === 0, "ohne Teil: ganze Merkliste");

console.log("Belege und Kostenstellen im Teil");
const kfp = { kosten: [{ name: "Honorare", posten: [{ id: "k1", bausteinId: "m" }, { id: "k2", bausteinId: "j" }] }, { name: "Werbung", posten: [{ id: "k3", bausteinId: "" }, { id: "k4", bausteinId: "weg" }] }] };
const belege = [
  { id: "b1", kostenstelle: "k1" },
  { id: "b2", kostenstelle: "k2" },
  { id: "b3", kostenstelle: "k3" },
  { id: "b4", kostenstelle: null },
  { id: "b5", kostenstelle: "k1" },
];
const bt = belegeFuerTeil(belege, maerz, kfp, teile);
pruefe(bt.belege.map((b) => b.id).join() === "b1,b5" && bt.ausgeblendet === 3, "nur Belege mit Kostenstelle von März");
pruefe(belegeFuerTeil(belege, null, kfp, teile).belege.length === 5, "ohne Teil: alle Belege");
const ks = kostenstellenVonTeil(maerz, kfp, teile);
pruefe(ks.size === 1 && ks.has("k1"), "Kostenstellen von März (Verweis auf gelöschten Teil zählt nicht)");
pruefe(kostenstellenVonTeil(null, kfp, teile).size === 0, "ohne Teil: keine");

if (fehler) {
  console.error(`\n${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\nAlle Prüfungen bestanden.");
