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
  einspielVorschlag,
  weitergabeInhalt,
  weitergabeUebernehmen,
  weitergabeAufbauPruefen,
  weitergabeName,
  freierOrgName,
  WEITERGABE_FELDER,
  NICHT_WEITERGEGEBEN,
  NAME_UNBEKANNT,
} from "../src/lib/organisationen.js";
import { readFileSync } from "node:fs";

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

console.log("Sicherung einspielen: Vorschlag je Organisation");
{
  const d = (/** @type {string} */ name) => JSON.stringify(tresor(name));
  const v = einspielVorschlag(
    [
      { id: "0000000000000000", vorhanden: true, daten: d("Ich privat") },
      { id: "aaaaaaaaaaaaaaaa", vorhanden: true, daten: d("Verein Blau") },
      { id: "bbbbbbbbbbbbbbbb", vorhanden: true, daten: null },
      { id: "cccccccccccccccc", vorhanden: false, daten: d("Duo") },
      { id: "dddddddddddddddd", vorhanden: false, daten: "kaputt{" },
      { id: "eeeeeeeeeeeeeeee", vorhanden: true, daten: d("Alter Name") },
    ],
    [
      { id: "0000000000000000", name: "Kulturverein Rot" },
      { id: "aaaaaaaaaaaaaaaa", name: "Verein Blau" },
      { id: "bbbbbbbbbbbbbbbb", name: "Verein Grün" },
      { id: "eeeeeeeeeeeeeeee", name: "Neuer Name" },
    ],
  );
  const erst = einspielVorschlag(
    [{ id: "0000000000000000", vorhanden: true, daten: d("Ich privat") }],
    [{ id: "0000000000000000", name: "Ich privat" }],
  );
  const erstGesperrt = einspielVorschlag([{ id: "0000000000000000", vorhanden: true, daten: null }], []);
  pruefe(v[0].wahl === "kopie" && v[0].andere && v[0].lokalName === "Kulturverein Rot", "erste Organisation, anderer Name: als Kopie, nie still ersetzen");
  pruefe(erst[0].wahl === "ersetzen" && !erst[0].andere, "erste Organisation, gleicher Name: ersetzen");
  pruefe(erstGesperrt[0].wahl === "kopie" && erstGesperrt[0].andere, "erste Organisation, Name unbekannt: als Kopie");
  pruefe(v[1].wahl === "ersetzen" && !v[1].andere, "zufällige ID, gleicher Name: ersetzen");
  pruefe(v[2].wahl === "ersetzen" && v[2].name === null, "zufällige ID, eigenes Passwort: ersetzen (ID ist eindeutig)");
  pruefe(v[5].wahl === "ersetzen" && !v[5].andere, "zufällige ID, seither umbenannt: ersetzen, keine Warnung");
  pruefe(v[3].wahl === "dazu" && v[3].name === "Duo", "neu: kommt dazu");
  pruefe(v[4].name === null && v[4].wahl === "dazu", "unlesbarer Inhalt bricht nichts");
}

console.log("Organisation weitergeben");
{
  // Jedes Feld von frischerTresor() – direkt aus dem Quelltext gelesen – steht
  // in GENAU einer Liste. Kommt ein neues dazu, schlägt dieser Test an, statt
  // dass es still weitergegeben (oder still weggelassen) wird.
  const quelle = readFileSync(new URL("../src/lib/tresorFormat.js", import.meta.url), "utf8");
  const block = quelle.split("export function frischerTresor()")[1].split("\n}")[0];
  const felder = [...block.matchAll(/^ {4}([A-Za-z]+):/gm)].map((m) => m[1]);
  pruefe(felder.length >= 15, `Felder von frischerTresor() gefunden (${felder.length})`);
  const ohneListe = felder.filter((f) => WEITERGABE_FELDER.includes(f) === NICHT_WEITERGEGEBEN.includes(f));
  pruefe(ohneListe.length === 0, `jedes Feld ist eingeordnet${ohneListe.length ? ": fehlt " + ohneListe.join(", ") : ""}`);

  const MARKE = "GEHEIM-MARKE";
  const quelleOrg = {
    ...frischerTresor(),
    organisation: { name: "Verein Blau", ordner: MARKE + "-ordner", ordnerUmzug: true },
    stammdaten: { person: { vorname: "Ada" }, bank: { iban: "CH00 1234" } },
    projekte: [{ id: "p1", name: "Fest", arbeitsdokumente: [{ url: "https://cryptpad.example/x" }] }],
    projektgruppen: [{ id: "g1", name: "Reihe" }],
    aktivesProjektId: "p1",
    modus: "team",
    sync: { adresse: MARKE, ausweis: MARKE + "-ausweis" },
    teamCa: MARKE + "-ca",
    katalogMeldungen: [{ text: MARKE + "-meldung" }],
    bekannteOrganisationen: [{ id: "x", name: MARKE + "-andere" }],
    zuletztGeoeffnet: MARKE + "-zeit",
    kuenftigesFeld: MARKE + "-neu",
  };
  const vorher = JSON.stringify(quelleOrg);
  const json = JSON.stringify(weitergabeInhalt(quelleOrg));
  pruefe(!json.includes(MARKE), "Team-Verbindung, Ordner, Meldungen, andere Organisationen und unbekannte Felder bleiben draußen");
  pruefe(json.includes("CH00 1234") && json.includes("Fest") && json.includes("Reihe") && json.includes("cryptpad"), "Stammdaten, Projekte, Gruppen und Links gehen mit");
  pruefe(JSON.stringify(quelleOrg) === vorher, "die Quelle bleibt unverändert");

  const roh = JSON.parse(json);
  pruefe(weitergabeName(roh) === "Verein Blau", "Name wird vorgeschlagen");
  const empfaenger = { ...frischerTresor(), netzModus: "knopf", einzelServer: "https://eigener.example", modus: "team", sync: { adresse: "EMPF" } };
  const r = weitergabeUebernehmen(frischerTresor(), empfaenger, roh, "Verein Blau");
  pruefe(r.ok && r.daten.projekte[0].name === "Fest" && r.daten.stammdaten.bank.iban === "CH00 1234", "Inhalt kommt an");
  pruefe(r.ok && r.daten.sync === null && r.daten.modus === "einzel" && r.daten.teamCa === null, "die neue Organisation hat keine Team-Verbindung");
  pruefe(r.ok && r.daten.netzModus === "knopf" && r.daten.einzelServer === "https://eigener.example", "Geräte-Einstellungen kommen vom Empfänger");
  pruefe(r.ok && r.daten.organisation.name === "Verein Blau" && !("ordner" in r.daten.organisation), "eigener Name, noch kein Ordner");

  // Fremde Daten: Aufbau wird geprüft, eingeschleuste Felder kommen nicht an.
  const boese = { ...roh, sync: { adresse: "x" }, modus: "team", katalogStand: { a: 1 }, organisation: { name: "X", ordner: ".." } };
  const rb = weitergabeUebernehmen(frischerTresor(), null, boese, "X");
  pruefe(rb.ok && rb.daten.sync === null && rb.daten.modus === "einzel" && JSON.stringify(rb.daten.katalogStand) === "{}" && !rb.daten.organisation.ordner, "eingeschleuste Felder werden ignoriert");
  const nein = (x, was) => pruefe(!weitergabeUebernehmen(frischerTresor(), null, x, "X").ok, was);
  nein(null, "leer: abgelehnt");
  nein({ ...roh, format: "anders" }, "fremdes Format: abgelehnt");
  nein({ ...roh, projekte: [null] }, "kaputtes Projekt: abgelehnt");
  nein({ ...roh, projekte: [{ id: "a", name: "A" }, { id: "a", name: "B" }] }, "doppelte Projekt-ID: abgelehnt");
  nein({ ...roh, projekte: "x" }, "Projekte keine Liste: abgelehnt");
  nein({ ...roh, stammdaten: [] }, "Stammdaten keine Gruppe: abgelehnt");
  nein({ ...roh, aktivesProjektId: 5 }, "aktives Projekt falscher Typ: abgelehnt");
  pruefe(weitergabeUebernehmen(frischerTresor(), null, { ...roh, version: 2 }, "X").fehler.includes("neueren"), "neuere Version: klare Meldung");
  pruefe(weitergabeName({ organisation: { name: 5 } }) === "", "unsinniger Name: leer");
  const mitLogo = (logo) => weitergabeUebernehmen(frischerTresor(), null, { ...roh, stammdaten: { ...roh.stammdaten, logo } }, "X").daten.stammdaten.logo;
  pruefe(mitLogo("data:image/png;base64,iVBORw0KGgo=") === "data:image/png;base64,iVBORw0KGgo=", "eingebettetes Logo bleibt");
  pruefe(mitLogo("https://tracker.example/pixel.png") === "", "Logo als Internet-Adresse wird verworfen (kein stiller Abruf)");
  pruefe(mitLogo("data:image/svg+xml;base64,PHN2Zz4=") === "" && mitLogo(5) === "", "SVG oder Unsinn als Logo wird verworfen");

  const liste = [{ id: "a", name: "Verein Blau" }, { id: "b", name: "Verein Blau (2)" }];
  pruefe(freierOrgName("Verein Blau", liste) === "Verein Blau (3)", "Name schon vergeben: freier Name");
  pruefe(freierOrgName("Duo", liste) === "Duo", "freier Name bleibt");
}

console.log("Aufbau einer übernommenen Organisation");
{
  // Ein ehrliches Projekt, wie es nach normalisieren() aussieht – einmal leer
  // (frisch angelegt), einmal reich gefüllt (Vorlage: tools/sync-waechter.mjs,
  // eigene Förderung nach dem Muster aus +page.svelte).
  const leer = () => ({
    id: "p0", name: "Neu", erstellt: "2026-11-17", gruppeId: null, vorgaengerId: null, archiviert: false,
    bausteine: [], fragebogen: null, merkliste: [],
    formular: { projekttitel: "", kurzbeschreibung: "", beschreibung: "", _eigeneFelder: [] },
    kfp: { kosten: [], finanzierung: [] }, kfpHinweisAusblenden: false,
    abrechnung: { belege: [], quellen: [], sachbericht: "" },
    antraege: {}, eigeneFoerderungen: [], interneFristen: [], katalogGhosts: [], katalogAktualisiert: [],
    arbeitsdokumente: [], ablageUrl: "", cryptpadHinweisAus: false,
  });
  const voll = () => ({
    ...leer(), id: "p1", name: "Klangraum Festival", gruppeId: "g1",
    bausteine: [{ id: "bs1", name: "Konzert März", beschreibung: "Text", ort: "Ort" }],
    fragebogen: { sparten: ["musik"], projektarten: [] },
    merkliste: ["de-musikfonds", "eigen-1"],
    formular: { projekttitel: "Klangraum", kurzbeschreibung: "k", beschreibung: "b", _eigeneFelder: [{ id: "ef1", titel: "Extra", text: "t" }] },
    kfp: {
      kosten: [{ name: "Personal", posten: [{ id: "k1", bezeichnung: "Honorar", erlaeuterung: "", betrag: "1000", bausteinId: "bs1" }] }],
      finanzierung: [{ name: "", posten: [{ foerderId: "de-musikfonds", bezeichnung: "", betrag: 1000, anteile: { bs1: "1000" } }] }],
    },
    antraege: {
      "de-musikfonds": { status: "abgeschickt", abdeckung: ["bs1"], statusFrei: "", offizielleFristen: ["2026-09-15"],
        eigeneFristen: [{ datum: "2026-09-01", titel: "Entwurf" }], kontakt: { ansprechpartner: "", email: "", telefon: "", notiz: "" },
        checkliste: [{ text: "Lebenslauf", status: "abgeschlossen", statusFrei: "", datei: "" }] },
      "eigen-1": { status: "in_bearbeitung", abdeckung: null, offizielleFristen: [], eigeneFristen: [] },
    },
    eigeneFoerderungen: [{ id: "eigen-1", eigen: true, name: "Lokaler Kulturtopf", foerdergeber: "Stadt", land: "ANDERES",
      beschreibung: "", webseite: "", foerderhoehe_text: "—", max_anteil_prozent: null, fristen: ["2026-10-01"],
      unvertraeglich_mit: [], checkliste_vorschlag: [],
      harte_kriterien: { wohnsitz: [], durchfuehrungsort: [], traegerschaft: ["einzelperson"], studentisch_erlaubt: true },
      weiche_kriterien: { sparten: [], projektarten: [], budget_min: null, budget_max: null, waehrung: "EUR", zeitpunkt: "fristen" } }],
    interneFristen: [{ id: "i1", datum: "2026-08-01", titel: "Kickoff" }],
    abrechnung: {
      belege: [{ id: "b1", nr: 1, datum: "2026-05-01", empfaenger: "E", zweck: "Z", brutto: "100", mwst_satz: "19",
        zahlungsart: "karte", kostenstelle: "k1", status: "erfasst", dateien: [{ name: "beleg.pdf" }],
        zuordnungen: [{ quelleId: "q1", betrag: "100" }], notiz: "" }],
      quellen: [{ id: "q1", typ: "foerderung", foerderId: "de-musikfonds", name: "Musikfonds", soll: "5000",
        nachweisFrist: "2027-03-01", sachbericht: "", dateien: [{ name: "auszug.pdf" }] }],
      sachbericht: "",
    },
    arbeitsdokumente: [{ id: "dok-1", titel: "Text", url: "https://cryptpad.fr/pad/#/2/x", foerderungen: ["de-musikfonds"] }],
  });
  const org = (projekte, gruppen = [{ id: "g1", name: "Klangraum" }]) => ({
    projekte, projektgruppen: gruppen, aktivesProjektId: projekte[0]?.id ?? null,
  });
  pruefe(weitergabeAufbauPruefen(org([leer()])).ok, "ehrlich: frisch angelegtes Projekt");
  pruefe(weitergabeAufbauPruefen(org([voll(), leer()])).ok, "ehrlich: reich gefülltes Projekt");
  pruefe(weitergabeAufbauPruefen(org([], [])).ok, "ehrlich: Organisation ohne Projekte");
  // Jede Förderung aus dem mitgelieferten Katalog muss als „Geist" (aus dem
  // Katalog verschwundene, aber gemerkte Förderung) durchgehen.
  const katalog = JSON.parse(readFileSync(new URL("../src/lib/daten/foerderungen.json", import.meta.url), "utf8")).foerderungen;
  const geister = katalog.filter((f) => !weitergabeAufbauPruefen(org([{ ...voll(), katalogGhosts: [f] }])).ok).map((f) => f.id);
  pruefe(katalog.length > 50 && geister.length === 0, `alle ${katalog.length} Katalog-Förderungen gehen als Geist durch${geister.length ? " – NICHT: " + geister.join(", ") : ""}`);
  pruefe(weitergabeAufbauPruefen(org([{ ...voll(), eigeneFoerderungen: katalog.slice(0, 5).map((f) => ({ ...f, eigen: true })) }])).ok,
    "Katalog-Einträge als eigene Förderungen gehen durch");
  // Fristen mit Hinweis kommen als Objekt (Katalog-Pflege, Team-Abgleich).
  const mitHinweis = voll();
  mitHinweis.eigeneFoerderungen[0].fristen = ["2026-10-01", { datum: "03-15", hinweis: "jährlich" }, { datum: "2026-12-01" }];
  mitHinweis.katalogGhosts = [{ ...mitHinweis.eigeneFoerderungen[0], id: "weg-1", eigen: undefined }];
  pruefe(weitergabeAufbauPruefen(org([mitHinweis])).ok, "Fristen mit Hinweis (Objekt) gehen durch");
  pruefe(weitergabeAufbauPruefen({ ...org([voll()]), kuenftig: 1, projekte: [{ ...voll(), neuesFeld: { x: 1 } }] }).ok,
    "unbekannte zusätzliche Felder stören nicht");

  // Jede dieser Varianten hat im Einbruchtest die Anzeige abstürzen lassen
  // (oder das Projekt-Menü festgehalten) – sie muss abgelehnt werden.
  /** @param {(p: any) => void} aendern @param {string} was */
  const kaputt = (aendern, was, gruppen) => {
    const p = voll();
    aendern(p);
    const r = weitergabeAufbauPruefen(org([p], gruppen));
    pruefe(!r.ok && r.fehler.includes("keine lesbare Organisation"), `${was}: abgelehnt${r.ok ? "" : " (" + r.stelle + ")"}`);
  };
  kaputt((p) => (p.kfp.kosten = [{}]), "Kostengruppe ohne Posten");
  kaputt((p) => (p.kfp.kosten[0].posten = null), "Posten-Liste null");
  kaputt((p) => (p.kfp.finanzierung = [null]), "Finanzierung mit leerem Eintrag");
  kaputt((p) => (p.abrechnung.belege = [5, "x", null]), "Belege mit Unsinn");
  kaputt((p) => (p.abrechnung.belege[0].zuordnungen = "x"), "Zuordnungen keine Liste");
  kaputt((p) => (p.formular._eigeneFelder = [null]), "Zusatzfeld leer");
  kaputt((p) => (p.interneFristen = [null]), "interne Frist leer");
  kaputt((p) => (p.bausteine = [{}]), "Teil ohne Namen");
  kaputt((p) => (p.bausteine[0].id = {}), "Teil-Kennung kein Text (Projekt-Menü hing)");
  kaputt((p) => delete p.eigeneFoerderungen[0].weiche_kriterien, "eigene Förderung ohne Kriterien");
  kaputt((p) => (p.antraege["de-musikfonds"] = "x"), "Antrag kein Objekt");
  kaputt((p) => (p.antraege["de-musikfonds"].checkliste = [null]), "Checkliste mit leerem Punkt");
  kaputt((p) => (p.arbeitsdokumente = [null]), "Arbeitsdokument leer");
  kaputt((p) => (p.merkliste = [null]), "Merkliste mit leerem Eintrag");
  kaputt((p) => (p.gruppeId = 7), "Gruppen-Kennung kein Text");
  kaputt((p) => (p.eigeneFoerderungen[0].fristen = [null]), "Frist leer");
  kaputt((p) => (p.eigeneFoerderungen[0].fristen = [{ datum: 5 }]), "Frist-Datum keine Zeichenfolge");
  kaputt(() => {}, "Projektgruppe ohne Namen", [{ id: "g1" }]);
  kaputt(() => {}, "Projektgruppe kein Objekt", [null]);
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
