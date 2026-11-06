// ============================================================
// Abrechnungs-Test: prüft die Bausteine in der Abrechnung – ohne die App zu
// starten. Wichtigste Punkte: Plan/Ist je Baustein geht ohne Rest in der
// Gesamtsumme auf, die Warnung kommt nur bei echten Abweichungen, und ohne
// Bausteine bleibt der Verwendungsnachweis wie bisher.
//
// Ausfuehren:  npm run abrechnung   (oder: node tools/abrechnung-test.mjs)
// ============================================================

import {
  belegBaustein,
  belegBausteinName,
  quelleAbdeckung,
  abdeckungsWarnung,
  bausteinUebersicht,
  kostenstellenUebersicht,
  kostenstellenNachKategorie,
  verwendungsnachweisAbschnitte,
} from "../src/lib/abrechnung.js";

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
const kfp = {
  kosten: [
    {
      name: "Honorare",
      posten: [
        { id: "k1", bezeichnung: "Gagen März", betrag: "1000", bausteinId: "maerz" },
        { id: "k2", bezeichnung: "Gagen Mai", betrag: "2000", bausteinId: "mai" },
      ],
    },
    { name: "Werbung", posten: [{ id: "k3", bezeichnung: "Plakate", betrag: "400", bausteinId: "" }] },
  ],
  finanzierung: [],
};
const beleg = (id, kostenstelle, brutto, zuordnungen = []) => ({
  id, nr: 1, datum: "2027-03-01", empfaenger: "Ensemble", zweck: id, brutto, kostenstelle, dateien: [], zuordnungen,
});
const bMaerz = beleg("b1", "k1", "800");
const bMai = beleg("b2", "k2", "1500");
const bPlakat = beleg("b3", "k3", "300");
const bOhne = beleg("b4", null, "50");
const bWeg = beleg("b5", "geloescht", "20");
const belege = [bMaerz, bMai, bPlakat, bOhne, bWeg];

const antraege = {
  kulturamt: { abdeckung: ["mai"] },
  fonds: { abdeckung: null },
  stiftung: { abdeckung: ["maerz", "gemeinsam"] },
  leer: { abdeckung: [] },
};
const quelle = (foerderId, name) => ({ id: "q-" + foerderId, typ: foerderId ? "foerderung" : "eigenmittel", foerderId, name, soll: "1000" });
const kulturamt = quelle("kulturamt", "Kulturamt");

console.log("Baustein eines Belegs:");
pruefe(belegBaustein(bMaerz, kfp, bausteine) === "maerz", "über die Kostenstelle");
pruefe(belegBaustein(bPlakat, kfp, bausteine) === "gemeinsam", "Kostenstelle ohne Baustein ist gemeinsam");
pruefe(belegBaustein(bOhne, kfp, bausteine) === null, "ohne Kostenstelle unbekannt");
pruefe(belegBaustein(bWeg, kfp, bausteine) === null, "gelöschte Kostenstelle unbekannt");
pruefe(belegBaustein(bMaerz, kfp, []) === null, "ohne Bausteine unbekannt");
pruefe(belegBausteinName(bMai, kfp, bausteine) === "Konzert Mai", "Name für die Anzeige");
pruefe(belegBausteinName(bOhne, kfp, bausteine) === "", "kein Name ohne Kostenstelle");

console.log("Abdeckung einer Geldquelle:");
pruefe(quelleAbdeckung(kulturamt, antraege, bausteine).join() === "mai", "Teil-Antrag");
pruefe(quelleAbdeckung(quelle("fonds", "Fonds"), antraege, bausteine) === null, "ganzes Projekt");
pruefe(quelleAbdeckung(quelle("", "Eintritt"), antraege, bausteine) === null, "Eigenmittel decken alles ab");
pruefe(quelleAbdeckung(quelle("unbekannt", "X"), antraege, bausteine) === null, "ohne Antrag: ganzes Projekt");
pruefe(quelleAbdeckung(kulturamt, antraege, []) === null, "ohne Bausteine: ganzes Projekt");

console.log("Warnung bei der Zuteilung:");
const w = abdeckungsWarnung(bMaerz, kulturamt, kfp, bausteine, antraege);
pruefe(w.includes("Kulturamt") && w.includes("Konzert Mai") && w.includes("„Konzert März"), "März-Beleg beim Mai-Förderer: Warnung nennt beide");
pruefe(abdeckungsWarnung(bMai, kulturamt, kfp, bausteine, antraege) === "", "passender Beleg: keine Warnung");
pruefe(abdeckungsWarnung(bPlakat, kulturamt, kfp, bausteine, antraege).includes("gemeinsamen Kosten"), "gemeinsamer Beleg ohne gemeinsame Kosten: Warnung");
pruefe(abdeckungsWarnung(bPlakat, quelle("stiftung", "Stiftung"), kfp, bausteine, antraege) === "", "gemeinsame Kosten mit abgedeckt: keine Warnung");
pruefe(abdeckungsWarnung(bOhne, kulturamt, kfp, bausteine, antraege) === "", "Beleg ohne Kostenstelle: keine Warnung");
pruefe(abdeckungsWarnung(bMaerz, quelle("fonds", "Fonds"), kfp, bausteine, antraege) === "", "ganzes Projekt: keine Warnung");
pruefe(abdeckungsWarnung(bMaerz, quelle("", "Eintritt"), kfp, bausteine, antraege) === "", "Eigenmittel: keine Warnung");
pruefe(abdeckungsWarnung(bMaerz, quelle("leer", "Leer"), kfp, bausteine, antraege).includes("noch keinen Teil"), "leere Abdeckung: eigene Warnung");
pruefe(abdeckungsWarnung(bMaerz, kulturamt, kfp, [], antraege) === "", "ohne Bausteine: nie eine Warnung");

console.log("Plan/Ist je Baustein:");
const ub = bausteinUebersicht(kfp, belege, bausteine);
const z = (id) => ub.find((x) => x.id === id);
pruefe(ub.map((x) => x.name).join() === "Konzert März,Konzert Mai,gemeinsam", "je Baustein eine Zeile, gemeinsam am Ende");
pruefe(z("maerz").plan === 1000 && z("maerz").ist === 800 && z("maerz").rest === 200 && z("maerz").anzahl === 1, "März: Plan, Ist, Rest, Anzahl");
pruefe(z("gemeinsam").plan === 400 && z("gemeinsam").ist === 300, "gemeinsam");
const ks = kostenstellenUebersicht(kfp, belege);
const planSumme = ub.reduce((s, x) => s + x.plan, 0);
const istSumme = ub.reduce((s, x) => s + x.ist, 0);
pruefe(planSumme === ks.planGesamt, "Plan der Bausteine ergibt den Gesamtplan");
pruefe(istSumme + ks.unzugeordnetSumme === ks.istGesamt, "Ist der Bausteine plus „Nicht zugeordnet“ ergibt das Gesamt-Ist");
pruefe(bausteinUebersicht(kfp, belege, []).length === 0, "ohne Bausteine keine Übersicht");

console.log("Kostenstellen-Auswahl:");
const auswahl = kostenstellenNachKategorie(kfp, bausteine);
pruefe(auswahl[0].posten[1].baustein === "Konzert Mai" && auswahl[1].posten[0].baustein === "gemeinsam", "nennt den Baustein");
pruefe(kostenstellenNachKategorie(kfp)[0].posten[0].baustein === "", "ohne Bausteine leer");

console.log("Verwendungsnachweis:");
const qMit = { ...kulturamt, dateien: [] };
const mitZuordnung = [
  beleg("b1", "k1", "800", [{ quelleId: qMit.id, betrag: "800" }]),
  beleg("b3", "k3", "300", [{ quelleId: qMit.id, betrag: "100" }]),
];
const ohneTeil = verwendungsnachweisAbschnitte(qMit, mitZuordnung, kfp, "Konzertreihe");
const mitTeil = verwendungsnachweisAbschnitte(qMit, mitZuordnung, kfp, "Konzertreihe", "", { bausteine, antraege });
const kopfzeilen = (n) => n.abschnitte.find((a) => a.ueberschrift === "Belegliste").tabelle.filter((r) => r.length === 1).map((r) => r[0]);
pruefe(kopfzeilen(ohneTeil).join(" | ") === "Kostenstelle 1.1 Gagen März | Kostenstelle 2.1 Plakate", "ohne Bausteine wie bisher");
pruefe(ohneTeil.untertitel.length === 2, "ohne Bausteine keine zusätzliche Zeile");
pruefe(kopfzeilen(mitTeil).join(" | ") === "Kostenstelle 1.1 Gagen März · Konzert März | Kostenstelle 2.1 Plakate · gemeinsam", "Kostenstelle nennt den Baustein");
pruefe(mitTeil.untertitel[2] === "Der Antrag umfasst Konzert Mai.", "Teil-Antrag: Umfang unter dem Titel");
const ganz = verwendungsnachweisAbschnitte({ ...qMit, foerderId: "fonds" }, mitZuordnung, kfp, "Konzertreihe", "", { bausteine, antraege });
pruefe(ganz.untertitel.length === 2, "ganzes Projekt: keine Umfang-Zeile");
const summe = (n) => n.abschnitte.find((a) => a.ueberschrift === "Belegliste").tabelle.at(-1).at(-1);
pruefe(summe(ohneTeil) === summe(mitTeil), "Summen bleiben gleich");

if (fehler) {
  console.error(`\n✗ ${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\n✓ Abrechnung OK: Bausteine gehen auf, gewarnt wird nur bei echten Abweichungen.");
