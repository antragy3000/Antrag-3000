// ============================================================
// Wächter: beweist, dass beim Synchronisieren KEINE sensiblen Felder
// das Gerät verlassen.
//
// Vorgehen: Ein Tresor wird mit klar markierten, erfundenen SENSIBLEN
// Werten gefüllt. Dann wird daraus mit boardAusTresor() das Sende-Paket
// gebaut. Taucht auch nur einer dieser Werte (oder ein verbotener
// Feldname) im Paket auf, schlägt der Wächter fehl (exit 1).
//
// Ausführen:  npm run waechter   (oder: node tools/sync-waechter.mjs)
// ============================================================

import { boardAusTresor, geteilteFoerdererAusTresor } from "../src/lib/sync.js";

// Unverwechselbare Markierungen für sensible Werte.
const S = {
  iban: "SENSIBEL_IBAN_DE00111122223333444455",
  bic: "SENSIBEL_BIC_XXXX",
  steuer: "SENSIBEL_STEUERNUMMER_12345",
  ustid: "SENSIBEL_USTID_DE999",
  privatMail: "SENSIBEL_PRIVATMAIL@example.invalid",
  privatTel: "SENSIBEL_TELEFON_0170",
  adresse: "SENSIBEL_STRASSE_Geheimweg_1",
  budget: "SENSIBEL_BUDGET_42000",
  beschreibung: "SENSIBEL_PROJEKTBESCHREIBUNG_geheim",
  ziele: "SENSIBEL_ZIELE_geheim",
  notiz: "SENSIBEL_NOTIZ_nur_fuer_mich",
  dateiname: "SENSIBEL_DATEINAME_Lebenslauf.pdf",
  eigenBeschr: "SENSIBEL_EIGENE_FOERDERUNG_BESCHREIBUNG",
};

// Verbotene Feldnamen (dürfen als Schlüssel nie im Board stehen).
const VERBOTENE_FELDNAMEN = [
  "stammdaten", "formular", "kfp", "fragebogen", "budget",
  "iban", "bic", "steuer", "ustid", "bank", "notiz",
  "beschreibung", "ziele", "datei",
];

// Tresor mit erfundenen sensiblen Daten.
const daten = {
  version: 2,
  stammdaten: {
    person: { vorname: "Max", nachname: "Muster", kuenstlername: "", organisation: "" },
    kontakt: { strasse: S.adresse, plz: "12345", ort: "Stadt", land: "DE", email: S.privatMail, telefon: S.privatTel, webseite: "" },
    bank: { kontoinhaber: "Max Muster", iban: S.iban, bic: S.bic, bank: "Geheimbank" },
    steuer: { steuernummer: S.steuer, ustid: S.ustid, finanzamt: "FA Stadt" },
  },
  projekte: [
    {
      id: "p1",
      name: "Klangraum Festival",
      fragebogen: { budget: S.budget, sparten: ["musik"] },
      merkliste: ["de-musikfonds", "eigen-1"],
      formular: {
        projekttitel: "Klangraum", kurzbeschreibung: S.beschreibung,
        beschreibung: S.beschreibung, ziele: S.ziele, zeitraum: "", ort: "", beteiligte: "",
      },
      kfp: {
        kosten: [{ name: "Personal", posten: [{ bezeichnung: "Honorar", erlaeuterung: "", betrag: S.budget }] }],
        finanzierung: [],
      },
      kfpHinweisAusblenden: false,
      antraege: {
        "de-musikfonds": {
          status: "abgeschickt",
          statusFrei: "",
          offizielleFristen: ["2026-09-15"],
          eigeneFristen: [{ datum: "2026-09-01", titel: "Teamabgabe Entwurf" }],
          kontakt: { ansprechpartner: "Frau Dr. Berg", email: "berg@musikfonds.example", telefon: "030 123456", notiz: S.notiz },
          checkliste: [
            { text: "Lebenslauf", status: "abgeschlossen", statusFrei: "", datei: S.dateiname },
          ],
        },
      },
      eigeneFoerderungen: [
        { id: "eigen-1", name: "Lokaler Kulturtopf", foerdergeber: "Stadt Musterstadt", beschreibung: S.eigenBeschr, fristen: ["2026-10-01"] },
      ],
      interneFristen: [{ id: "i1", datum: "2026-08-01", titel: "Kickoff-Treffen" }],
    },
  ],
  aktivesProjektId: "p1",
};

const board = boardAusTresor(daten);
// Auch das Paket der team-geteilten EIGENEN Foerderer wird geprueft –
// die freie Beschreibung (S.eigenBeschr) darf darin NICHT auftauchen.
const geteilt = geteilteFoerdererAusTresor(daten);
const json = JSON.stringify(board) + "\n" + JSON.stringify(geteilt);
const jsonKlein = json.toLowerCase();

let leck = false;

// 1) Kein sensibler WERT darf im Paket auftauchen.
for (const [name, wert] of Object.entries(S)) {
  if (json.includes(wert)) {
    console.error(`  ✗ LECK: sensibler Wert "${name}" (${wert}) steht im Sende-Paket.`);
    leck = true;
  }
}

// 2) Kein verbotener FELDNAME darf als Schlüssel vorkommen.
for (const feld of VERBOTENE_FELDNAMEN) {
  if (jsonKlein.includes(`"${feld}"`)) {
    console.error(`  ✗ LECK: verbotener Feldname "${feld}" steht im Sende-Paket.`);
    leck = true;
  }
}

// 3) Erwartete unkritische Inhalte SOLLTEN vorhanden sein (Gegenprobe,
//    damit der Test nicht versehentlich „leer" durchläuft).
const erwartet = ["Klangraum Festival", "de-musikfonds", "abgeschickt", "Frau Dr. Berg", "Kickoff-Treffen", "Lokaler Kulturtopf"];
for (const e of erwartet) {
  if (!json.includes(e)) {
    console.error(`  ✗ FEHLT: erwarteter unkritischer Wert "${e}" ist NICHT im Paket – Test unzuverlässig.`);
    leck = true;
  }
}

if (leck) {
  console.error("\nWächter FEHLGESCHLAGEN: Es würden sensible Daten synchronisiert.\n");
  process.exit(1);
}

console.log("✓ Wächter OK: keine sensiblen Werte und keine verbotenen Felder im Sende-Paket.");
console.log("  (Die erwarteten unkritischen Board-Daten sind enthalten.)\n");
console.log("Zur Kontrolle – so sehen die Sende-Pakete aus:\n");
console.log("Board:\n" + JSON.stringify(board, null, 2));
console.log("\nGeteilte eigene Förderer (ohne Beschreibung):\n" + JSON.stringify(geteilt, null, 2));
process.exit(0);
