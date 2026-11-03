// ============================================================
// Sachbericht-Test: prüft, dass je Geldquelle der richtige Sachbericht gilt
// (eigener Text vor gemeinsamer Vorlage) und dass kein Sachbericht ins
// Team-Sende-Paket gerät – ohne die App zu starten.
//
// Ausfuehren:  npm run sachbericht   (oder: node tools/sachbericht-test.mjs)
// ============================================================

import { hatEigenenSachbericht, sachberichtFuerQuelle } from "../src/lib/sachbericht.js";
import { boardAusTresor } from "../src/lib/sync.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

const VORLAGE = "SENSIBEL_GEMEINSAME_VORLAGE";
const EIGEN = "SENSIBEL_EIGENER_BERICHT_STIFTUNG";

const mitEigenem = { id: "q1", name: "Stiftung", sachbericht: EIGEN };
const ohneEigenen = { id: "q2", name: "Stadt", sachbericht: "" };
const altStand = { id: "q3", name: "Altstand" };
const nurLeerzeichen = { id: "q4", name: "Leer", sachbericht: "  \n " };

console.log("Welcher Text gilt?");
pruefe(hatEigenenSachbericht(mitEigenem), "eigener Text wird erkannt");
pruefe(!hatEigenenSachbericht(ohneEigenen), "leeres Feld ist kein eigener Text");
pruefe(!hatEigenenSachbericht(nurLeerzeichen), "nur Leerzeichen sind kein eigener Text");
pruefe(!hatEigenenSachbericht(altStand), "älterer Stand ohne Feld bricht nicht ab");
pruefe(sachberichtFuerQuelle(mitEigenem, VORLAGE) === EIGEN, "eigener Text geht vor Vorlage");
pruefe(sachberichtFuerQuelle(ohneEigenen, VORLAGE) === VORLAGE, "ohne eigenen Text gilt die Vorlage");
pruefe(sachberichtFuerQuelle(altStand, VORLAGE) === VORLAGE, "älterer Stand nutzt weiter die Vorlage");
pruefe(sachberichtFuerQuelle(nurLeerzeichen, VORLAGE) === VORLAGE, "nur Leerzeichen: Vorlage gilt");
pruefe(sachberichtFuerQuelle(ohneEigenen) === "", "ohne beides: kein Text");

console.log("Bleibt alles auf dem Gerät?");
const paket = JSON.stringify(
  boardAusTresor({
    projekte: [{ id: "p1", name: "Projekt", merkliste: [], antraege: {}, abrechnung: { belege: [], quellen: [mitEigenem], sachbericht: VORLAGE } }],
  })
);
pruefe(paket.includes("Projekt"), "Gegenprobe: das Projekt selbst ist im Sende-Paket");
pruefe(!paket.includes(EIGEN), "eigener Sachbericht ist NICHT im Sende-Paket");
pruefe(!paket.includes(VORLAGE), "gemeinsame Vorlage ist NICHT im Sende-Paket");

if (fehler > 0) {
  console.error(`\n❌ Sachbericht-Test: ${fehler} Fehler.`);
  process.exit(1);
}
console.log("\n✅ Sachbericht-Test bestanden.");
