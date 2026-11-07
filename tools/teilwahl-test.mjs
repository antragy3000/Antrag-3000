// ============================================================
// Teil-Wahl-Test: prüft, welcher Teil im Projektmenü als gewählt gilt und
// wie ein Teil direkt im Menü angelegt wird – ohne die App zu starten.
//
// Ausfuehren:  npm run teilwahl   (oder: node tools/teilwahl-test.mjs)
// ============================================================

import { teilImBlick, teilAnlegen, neueTeilId } from "../src/lib/teilwahl.js";

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

if (fehler) {
  console.error(`\n${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\nAlle Prüfungen bestanden.");
