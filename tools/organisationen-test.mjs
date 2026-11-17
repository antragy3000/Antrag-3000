// ============================================================
// Organisationen-Test: prüft Namen, die Liste im Umschalter, was jeder
// Tresor über die anderen weiß und wie eine neue Organisation entsteht –
// ohne die App zu starten.
//
// Ausfuehren:  npm run organisationen   (oder: node tools/organisationen-test.mjs)
// ============================================================

import {
  orgName,
  orgListe,
  bekannteAbgleichen,
  namePruefen,
  neueOrgDaten,
  startOrg,
  orgFelderErgaenzen,
  orgOrdnerVon,
  offeneOrgFinden,
  NAME_UNBEKANNT,
} from "../src/lib/organisationen.js";

// Ausschnitt von frischerTresor() (tresorFormat.js lädt Vite-Module und läuft
// darum nicht direkt in Node).
const frischerTresor = () => ({
  version: 2,
  stammdaten: {
    person: { vorname: "", nachname: "", kuenstlername: "", organisation: "" },
    kontakt: {},
    bank: {},
    steuer: {},
    logo: "",
  },
  projekte: [],
  projektgruppen: [],
  aktivesProjektId: null,
  modus: "einzel",
  modusHinweisAus: false,
  netzModus: null,
  einzelServer: "https://sync.antrag3000.de",
  sync: null,
  teamCa: null,
  katalogMeldungen: [],
  katalogStand: {},
  katalogFeldDiff: {},
  organisation: { name: "" },
  bekannteOrganisationen: [],
});

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

const tresor = (name = "", person = {}) => {
  const d = frischerTresor();
  d.organisation.name = name;
  Object.assign(d.stammdaten.person, person);
  return d;
};

console.log("Namen");
pruefe(orgName(tresor("Kulturverein Blau")) === "Kulturverein Blau", "eigener Name geht vor");
pruefe(orgName(tresor("", { organisation: "Ensemble Nord" })) === "Ensemble Nord", "sonst Organisation aus den Stammdaten");
pruefe(orgName(tresor("", { kuenstlername: "Mira K." })) === "Mira K.", "sonst Künstlername");
pruefe(orgName(tresor("", { vorname: "Mira", nachname: "Kern" })) === "Mira Kern", "sonst Vor- und Nachname");
pruefe(orgName(tresor()) === "Meine Organisation", "leerer Tresor bekommt einen Platzhalter");

console.log("Ältere Tresore");
const alt = frischerTresor();
delete alt.organisation;
delete alt.bekannteOrganisationen;
pruefe(orgFelderErgaenzen(alt) === true, "Ergänzung wird gemeldet (dann speichert die App)");
pruefe(alt.organisation?.name === "" && Array.isArray(alt.bekannteOrganisationen), "Name und Liste werden ergänzt");
pruefe(orgFelderErgaenzen(alt) === false, "zweiter Lauf ändert nichts");

console.log("Liste im Umschalter");
const privat = { id: "aaaaaaaaaaaaaaaa", daten: tresor("Ich privat") };
const verein = { id: "bbbbbbbbbbbbbbbb", daten: tresor("Kulturverein Blau") };
const gesperrtId = "cccccccccccccccc";
const fremdId = "dddddddddddddddd";
verein.daten.bekannteOrganisationen = [{ id: gesperrtId, name: "Duo Lichtung" }];
const liste = orgListe([privat, verein], [privat.id, verein.id, gesperrtId, fremdId], verein.id);
pruefe(liste.length === 4, "alle vier Organisationen stehen in der Liste");
pruefe(liste.map((e) => e.name).join(" | ") === `Duo Lichtung | ${NAME_UNBEKANNT} | Ich privat | Kulturverein Blau`, "sortiert nach Name");
pruefe(liste.find((e) => e.id === gesperrtId)?.offen === false, "gesperrte ist als gesperrt markiert");
pruefe(liste.find((e) => e.id === verein.id)?.aktiv === true, "aktive ist markiert");

console.log("Bekannte Namen abgleichen");
const geaendert = bekannteAbgleichen([privat, verein], [privat.id, verein.id, gesperrtId, fremdId]);
pruefe(geaendert.includes(privat.id), "privat lernt die anderen kennen");
pruefe(
  JSON.stringify(privat.daten.bekannteOrganisationen) ===
    JSON.stringify([{ id: gesperrtId, name: "Duo Lichtung" }, { id: verein.id, name: "Kulturverein Blau" }]),
  "privat kennt Verein und die gesperrte Duo Lichtung, nicht aber sich selbst",
);
pruefe(!privat.daten.bekannteOrganisationen.some((b) => b.id === fremdId), "namenlose gesperrte wird nicht als Name gespeichert");
pruefe(bekannteAbgleichen([privat, verein], [privat.id, verein.id, gesperrtId, fremdId]).length === 0, "zweiter Abgleich ändert nichts");
verein.daten.organisation.name = "Verein Blau e. V.";
pruefe(bekannteAbgleichen([privat, verein], [privat.id, verein.id]).join() === `${privat.id},${verein.id}`, "Umbenennen und Verschwinden erreichen beide");
pruefe(privat.daten.bekannteOrganisationen.some((b) => b.name === "Verein Blau e. V."), "neuer Name steht in der Liste");
pruefe(!verein.daten.bekannteOrganisationen.some((b) => b.id === gesperrtId), "verschwundene (Duo Lichtung) fliegt raus");

console.log("Namen prüfen");
const l2 = orgListe([privat, verein], [privat.id, verein.id], privat.id);
pruefe(namePruefen("  ", l2).ok === false && namePruefen("  ", l2).fehler === "", "leer: still nicht ok");
pruefe(namePruefen("ich  PRIVAT", l2).ok === false, "gleicher Name (Groß/klein, Leerraum) wird abgewiesen");
pruefe(namePruefen("Ich privat", l2, privat.id).ok === true, "eigener Name beim Umbenennen ist erlaubt");
pruefe(namePruefen("  Neue   Gruppe ", l2).name === "Neue Gruppe", "Leerraum wird geglättet");
pruefe(namePruefen("x".repeat(81), l2).ok === false, "zu lange Namen werden abgewiesen");

console.log("Neue Organisation");
const vorlage = tresor("Ich privat", { vorname: "Mira", nachname: "Kern" });
vorlage.netzModus = "knopf";
vorlage.katalogStand = { f1: "2026-10-01" };
vorlage.sync = { adresse: "team.antrag3000.de:8443", ausweisPem: "GEHEIM" };
vorlage.projekte.push({ id: "p1", name: "Sommerfest" });
const neu = neueOrgDaten(frischerTresor(), vorlage, "Kulturverein Blau");
pruefe(neu.organisation.name === "Kulturverein Blau", "bekommt ihren Namen");
pruefe(neu.netzModus === "knopf", "Netz-Wahl des Geräts wird übernommen (keine erneute Frage)");
pruefe(neu.katalogStand.f1 === "2026-10-01" && neu.katalogStand !== vorlage.katalogStand, "Stand der Förder-Datenbank als Kopie");
pruefe(neu.sync === null && neu.teamCa === null, "keine Team-Anbindung (der Ausweis gehört zur alten Organisation)");
pruefe(neu.projekte.length === 0 && neu.stammdaten.person.vorname === "", "Projekte und Stammdaten leer");

pruefe(neu.organisation.ordner === undefined, "übernimmt nicht den Ordner der Vorlage");

console.log("Ordner der Organisation");
const mitOrdner = tresor("Verein");
pruefe(orgOrdnerVon(mitOrdner) === "", "ohne Ordner: direkt in der Wurzel (Umzug steht noch aus)");
mitOrdner.organisation.ordner = "Verein";
pruefe(orgOrdnerVon(mitOrdner) === "Verein", "mit Ordner: der eigene");
mitOrdner.organisation.ordnerUmzug = true;
pruefe(orgOrdnerVon(mitOrdner) === "", "Umzug offen: die Dateien liegen noch in der Wurzel");
delete mitOrdner.organisation.ordnerUmzug;
pruefe(orgOrdnerVon(mitOrdner) === "Verein", "Umzug fertig: wieder der eigene");
pruefe(orgOrdnerVon(null) === "" && orgOrdnerVon({}) === "", "ohne Daten: kein Absturz");
const krumm = tresor("Verein");
krumm.organisation.ordner = 42;
pruefe(orgOrdnerVon(krumm) === "", "unbrauchbarer Wert wird ignoriert");
orgFelderErgaenzen(mitOrdner);
pruefe(mitOrdner.organisation.ordner === "Verein", "Ergänzen älterer Felder lässt den Ordner stehen");

console.log("Netz-Ergebnis nach einem Wechsel");
{
  const a = { id: "aaaaaaaaaaaaaaaa", daten: tresor("A") };
  const b = { id: "bbbbbbbbbbbbbbbb", daten: tresor("B") };
  // Aufruf startet in A; inzwischen ist B aktiv, A liegt unter den anderen.
  const ziel = offeneOrgFinden(a.id, b.id, b.daten, [a]);
  pruefe(ziel?.daten === a.daten && ziel.aktiv === false, "landet bei A, obwohl B jetzt aktiv ist");
  pruefe(offeneOrgFinden(b.id, b.id, b.daten, [a])?.aktiv === true, "die aktive wird als aktiv erkannt");
  pruefe(offeneOrgFinden(a.id, b.id, b.daten, []) === null, "A inzwischen gesperrt/entfernt: Ergebnis wird verworfen");
  pruefe(offeneOrgFinden(a.id, null, null, []) === null, "ganz gesperrt: verworfen");
  pruefe(offeneOrgFinden(null, b.id, b.daten, [a]) === null, "ohne ID: nichts");
}

console.log("Start-Organisation");
privat.daten.zuletztGeoeffnet = "2026-11-07T10:00:00.000Z";
verein.daten.zuletztGeoeffnet = "2026-11-07T12:00:00.000Z";
pruefe(startOrg([privat, verein]) === verein.id, "die zuletzt benutzte wird aktiv");
delete privat.daten.zuletztGeoeffnet;
delete verein.daten.zuletztGeoeffnet;
pruefe(startOrg([verein, privat]) === privat.id, "ohne Zeitstempel die erste nach Name");
pruefe(startOrg([]) === null, "ohne offene Organisation: keine");

if (fehler) {
  console.error(`\n${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\nAlle Prüfungen bestanden.");
