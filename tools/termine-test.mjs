// ============================================================
// Termine-Test: prüft, dass termine.js alle vier Termin-Arten richtig
// einsammelt, sortiert und nach Tagen gruppiert – ohne die App zu starten.
//
// Ausfuehren:  npm run termine   (oder: node tools/termine-test.mjs)
// ============================================================

import { termineSammeln, termineJeTag, naechsteTermine, tageBis, isoTag } from "../src/lib/termine.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

// Fester „heute"-Tag, damit der Test jedes Jahr gleich laeuft.
const HEUTE = new Date(2026, 9, 27); // 27.10.2026

const gemerkte = [
  { id: "f1", name: "Kulturfonds", fristen: ["2026-11-15", "03-01"] },
  { id: "f2", name: "Stadt-Atelier", fristen: [] },
];
const antraege = {
  f2: { offizielleFristen: ["2026-12-01"], eigeneFristen: [{ datum: "2026-11-20", titel: "Budget fertig" }, "2026-11-25"] },
};
const interneFristen = [{ id: "i1", datum: "2026-10-30", titel: "Probe" }, { id: "i2", datum: "", titel: "ohne Datum" }];
const quellen = [
  { id: "q1", name: "Pro Helvetia", nachweisFrist: "2027-02-28" },
  { id: "q2", name: "Eigenmittel" },
];
const teamBoard = [
  { projekt_id: "meins", inhalt: { name: "Mein Projekt", interneFristen: [{ id: "x", datum: "2026-11-01", titel: "doppelt" }] } },
  {
    projekt_id: "andere",
    inhalt: {
      name: "Tanzstück",
      eintraege: [{ foerderungId: "f9", eigenesLabel: "Tanzfonds", offizielleFristen: ["2026-11-10"], eigeneFristen: [] }],
      interneFristen: [{ id: "t1", datum: "2026-11-05", titel: "Bühnenplan" }],
    },
  },
];

const termine = termineSammeln({
  gemerkte, antraege, interneFristen, quellen, teamBoard, meineProjektIds: ["meins"], heute: HEUTE,
});

console.log("Einsammeln:");
const arten = (a) => termine.filter((t) => t.art === a);
pruefe(arten("foerderung").length === 5, "5 Förder-Fristen (2 offizielle f1, 1 offizielle + 2 eigene f2)");
pruefe(arten("foerderung").some((t) => t.datum === "2027-03-01"), "jährliche Frist 03-01 wird auf 01.03.2027 aufgelöst");
pruefe(!arten("foerderung").some((t) => t.datum === "2026-11-15" && t.titel === "Stadt-Atelier"), "f2 nutzt offizielleFristen aus dem Antrag, nicht f.fristen");
pruefe(arten("intern").length === 1, "interne Frist ohne Datum wird übersprungen");
pruefe(arten("nachweis").length === 1 && arten("nachweis")[0].ziel.bereich === "geldquellen", "Verwendungsnachweis nur für Quelle mit Datum, Ziel = Geldquellen");
pruefe(arten("team").length === 2, "2 Team-Fristen vom anderen Gerät");
pruefe(!termine.some((t) => t.titel === "doppelt"), "eigenes Board-Projekt wird nicht doppelt gezeigt");
pruefe(arten("team").every((t) => t.projektName === "Tanzstück"), "Team-Fristen tragen den Projektnamen");

console.log("Sortierung und Gruppierung:");
const daten = termine.map((t) => t.datum);
pruefe(daten.join() === [...daten].sort().join(), "Termine sind nach Datum sortiert");
pruefe(termineJeTag(termine)["2026-11-15"]?.length === 1, "termineJeTag gruppiert nach Tag");
const naechste = naechsteTermine(termine, 3, HEUTE);
pruefe(naechste.length === 3 && naechste[0].datum === "2026-10-30", "naechsteTermine: die 3 nächsten ab heute");

console.log("Datums-Helfer:");
pruefe(tageBis("2026-10-27", HEUTE) === 0, "heute = 0 Tage");
pruefe(tageBis("2026-11-03", HEUTE) === 7, "über die Zeitumstellung (25.10.) hinweg genau 7 Tage");
pruefe(tageBis("2026-10-20", HEUTE) === -7, "vergangene Tage sind negativ");
pruefe(isoTag(new Date(2026, 0, 5)) === "2026-01-05", "isoTag mit führenden Nullen");
pruefe(termineSammeln().length === 0, "ohne Angaben: leere Liste, kein Absturz");

if (fehler === 0) {
  console.log("\n✅ Termine-Test bestanden.");
  process.exit(0);
} else {
  console.error(`\n❌ Termine-Test FEHLGESCHLAGEN: ${fehler} Problem(e).`);
  process.exit(1);
}
