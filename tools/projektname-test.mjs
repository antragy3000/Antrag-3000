// ============================================================
// Projektname-Test: prüft, dass zwei Projekte nie denselben Ordner bekommen –
// ohne die App zu starten.
//
// Ausfuehren:  npm run projektname   (oder: node tools/projektname-test.mjs)
// ============================================================

import { ordnerName, projektNamePruefen } from "../src/lib/projektname.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

const mai = { id: "a", name: "Konzert Mai", archiviert: false };
const werkschau = { id: "b", name: "Werkschau 2025", archiviert: true };
const projekte = [mai, werkschau];

console.log("Ordnername wie im Rust-Kern (ordner.rs, bereinigen):");
pruefe(ordnerName("Konzert:Mai") === "Konzert_Mai", "verbotene Zeichen werden zu _");
pruefe(ordnerName("  Konzert.. ") === "Konzert", "Leerraum und Punkte am Rand fallen weg");
pruefe(ordnerName("...") === "", "nur Punkte ergeben keinen Ordnernamen");
pruefe(ordnerName("CON") === "_CON", "reservierter Gerätename bekommt ein _ davor");
pruefe(ordnerName("con.txt") === "_con.txt", "auch mit Endung und kleingeschrieben");
pruefe(ordnerName("Concert") === "Concert", "ähnliche Namen bleiben unverändert");

console.log("Neues Projekt:");
pruefe(projektNamePruefen("Konzert Juni", projekte).ok, "freier Name ist erlaubt");
pruefe(projektNamePruefen("  Konzert Juni ", projekte).name === "Konzert Juni", "Name wird gesäubert");
pruefe(!projektNamePruefen("Konzert Mai", projekte).ok, "gleicher Name ist abgelehnt");
pruefe(!projektNamePruefen("konzert mai", projekte).ok, "andere Groß-/Kleinschreibung ist abgelehnt");
pruefe(!projektNamePruefen("Konzert Mai.", projekte).ok, "Punkt am Ende ist abgelehnt");
pruefe(!projektNamePruefen("Werkschau 2025", projekte).ok, "auch archivierte Projekte belegen ihren Ordner");
pruefe(projektNamePruefen("Werkschau 2025", projekte).fehler.includes("im Archiv"), "Meldung nennt das Archiv");
const leer = projektNamePruefen("   ", projekte);
pruefe(!leer.ok && leer.fehler === "", "leere Eingabe: nicht erlaubt, aber ohne Meldung");
const punkte = projektNamePruefen("...", projekte);
pruefe(!punkte.ok && punkte.fehler !== "", "ungültiger Name: nicht erlaubt, mit Meldung");

const sonderzeichen = [{ id: "c", name: "Konzert_Mai" }];
pruefe(!projektNamePruefen("Konzert:Mai", sonderzeichen).ok, "verschiedene Namen, derselbe Ordner: abgelehnt");

console.log("Umbenennen:");
pruefe(projektNamePruefen("Konzert Mai", projekte, mai).ok, "eigener Name bleibt erlaubt");
pruefe(projektNamePruefen("KONZERT MAI", projekte, mai).ok, "nur die Schreibweise ändern ist erlaubt");
pruefe(projektNamePruefen("Konzert Juni", projekte, mai).ok, "freier Name ist erlaubt");
pruefe(!projektNamePruefen("Werkschau 2025", projekte, mai).ok, "Name eines anderen Projekts ist abgelehnt");

console.log("Ältere Stände mit doppeltem Namen:");
const doppelt = [
  { id: "x", name: "Festival" },
  { id: "y", name: "Festival" },
];
pruefe(projektNamePruefen("Festival", doppelt, doppelt[1]).ok, "Projekt bleibt bearbeitbar");
pruefe(projektNamePruefen("Festival 2", doppelt, doppelt[1]).ok, "Projekt lässt sich auf einen freien Namen umbenennen");
pruefe(!projektNamePruefen("Festival", doppelt).ok, "kein drittes Projekt mit diesem Namen");

if (fehler) {
  console.error(`\n✗ ${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\n✓ Projektnamen OK: Zwei Projekte bekommen nie denselben Ordner.");
