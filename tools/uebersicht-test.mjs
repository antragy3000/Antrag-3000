// ============================================================
// Übersicht-Test: prüft die Regeln der Startseite (uebersicht.js) –
// ohne die App zu starten.
//
// Ausfuehren:  npm run uebersicht   (oder: node tools/uebersicht-test.mjs)
// ============================================================

import { termineSammeln } from "../src/lib/termine.js";
import { alsNaechstes, antraegeNachStatus, abrechnungStand, tageText, monatsRaster } from "../src/lib/uebersicht.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

const HEUTE = new Date(2026, 10, 3); // 03.11.2026

const gemerkte = [
  { id: "offen", name: "Offen", fristen: ["2026-11-08", "2027-03-01"] },
  { id: "weg", name: "Abgeschickt", fristen: ["2026-11-10"] },
  { id: "nach", name: "Nachbessern", fristen: [] },
  { id: "alt", name: "Verpasst", fristen: ["2026-10-01"] },
];
const antraege = {
  weg: { status: "abgeschickt", checkliste: [{ text: "Vergessen", status: "noch_nicht" }] },
  nach: { status: "nachbessern", checkliste: [{ text: "Offerte", status: "anfragen" }, { text: "CV", status: "abgeschlossen" }] },
};
const interneFristen = [
  { id: "i-bald", datum: "2026-11-30", titel: "Probe" },
  { id: "i-frisch", datum: "2026-11-01", titel: "Kürzlich vorbei" },
  { id: "i-uralt", datum: "2026-08-01", titel: "Längst vorbei" },
];
const quellen = [{ id: "q1", name: "Stiftung", nachweisFrist: "2026-11-29" }];
const teamBoard = [{ projekt_id: "x", inhalt: { name: "Anderes", interneFristen: [{ id: "t", datum: "2026-11-04", titel: "Team" }] } }];

const termine = termineSammeln({ gemerkte, antraege, interneFristen, quellen, teamBoard, heute: HEUTE });
const { eintraege, gesamt } = alsNaechstes({ termine, gemerkte, antraege, anzahl: 50, heute: HEUTE });
const titel = eintraege.map((e) => e.titel);

console.log("Als Nächstes – was erscheint:");
pruefe(titel.includes("Offen"), "offene Förder-Frist erscheint");
pruefe(titel.filter((t) => t === "Offen").length === 1, "von mehreren Runden nur die nächste");
pruefe(!titel.includes("Abgeschickt"), "abgeschickter Antrag erinnert nicht mehr");
pruefe(!titel.includes("Verpasst"), "verpasste Runde ist keine Aufgabe");
pruefe(!titel.includes("Team"), "Team-Fristen fehlen");
pruefe(titel.includes("Kürzlich vorbei"), "interne Frist bleibt kurz nach Ablauf sichtbar");
pruefe(!titel.includes("Längst vorbei"), "alte interne Frist verschwindet nach 30 Tagen");
pruefe(eintraege.some((e) => e.art === "status" && e.titel === "Nachbessern"), "Status „nachbessern“ erscheint");
pruefe(eintraege.filter((e) => e.art === "check").length === 1, "nur Checklisten-Punkte „muss angefragt werden“");

console.log("Als Nächstes – Reihenfolge und Begrenzung:");
const reihe = eintraege.map((e) => e.titel + (e.art === "check" ? " (check)" : ""));
pruefe(reihe[0] === "Kürzlich vorbei", "überfällig steht ganz oben");
pruefe(reihe[1] === "Offen", "dann bald fällig (≤ 14 Tage)");
pruefe(reihe[2] === "Nachbessern", "dann nachbessern");
pruefe(reihe[3] === "Nachbessern (check)", "dann anfragen");
pruefe(reihe[4] === "Stiftung" && reihe[5] === "Probe", "dann Fristen mit viel Zeit, nach Datum");
const kurz = alsNaechstes({ termine, gemerkte, antraege, anzahl: 2, heute: HEUTE });
pruefe(kurz.eintraege.length === 2 && kurz.gesamt === gesamt, "begrenzt die Anzeige, zählt aber alle");

console.log("Anträge und Abrechnung:");
const st = antraegeNachStatus(gemerkte, antraege);
pruefe(st.find((s) => s.key === "in_bearbeitung")?.anzahl === 2, "ohne Eintrag zählt als „in Bearbeitung“");
pruefe(st.map((s) => s.key).join() === "in_bearbeitung,abgeschickt,nachbessern", "feste Reihenfolge, leere Status fehlen");
// Vereinfachte Rechenfunktionen (die echten aus abrechnung.js brauchen den Katalog).
const zugeordnet = (b) => (b.zuordnungen ?? []).reduce((s, z) => s + Number(z.betrag), 0);
const rechner = {
  belegFrei: (b) => Number(b.brutto) - zugeordnet(b),
  quelleSoll: (q) => Number(q.soll),
  zugeordnetJeQuelle: (belege) => {
    const m = new Map();
    for (const b of belege) for (const z of b.zuordnungen ?? []) m.set(z.quelleId, (m.get(z.quelleId) ?? 0) + Number(z.betrag));
    return m;
  },
};
pruefe(abrechnungStand({ belege: [], quellen: [] }, rechner).sichtbar === false, "ohne Belege und Quellen keine Kachel");
pruefe(abrechnungStand(undefined, rechner).sichtbar === false, "alte Projekte ohne Abrechnung stürzen nicht ab");
const ab = abrechnungStand({
  quellen: [{ id: "q1", name: "Stiftung", soll: "1000" }],
  belege: [
    { id: "b1", brutto: "100", zuordnungen: [{ quelleId: "q1", betrag: "100" }] },
    { id: "b2", brutto: "50", zuordnungen: [{ quelleId: "q1", betrag: "20" }] },
  ],
}, rechner);
pruefe(ab.belegeOffen === 1, "teilweise zugeordneter Beleg zählt als offen");
pruefe(ab.quellen[0].soll === 1000 && ab.quellen[0].zugeordnet === 120, "Soll und Zugeordnet je Quelle");

console.log("Texte und Raster:");
pruefe(tageText(0) === "heute" && tageText(1) === "morgen" && tageText(5) === "in 5 Tagen", "Text für künftige Tage");
pruefe(tageText(-1) === "seit 1 Tag überfällig" && tageText(-3) === "seit 3 Tagen überfällig", "Text für vergangene Tage");
const nov = monatsRaster(2026, 10); // November 2026 beginnt an einem Sonntag
pruefe(nov.slice(0, 6).every((z) => z === null) && nov[6].iso === "2026-11-01", "Monat beginnt in der richtigen Spalte (Montag zuerst)");
pruefe(nov.length % 7 === 0 && nov.filter(Boolean).length === 30, "volle Wochen, 30 Tage");
pruefe(monatsRaster(2028, 1).filter(Boolean).length === 29, "Schaltjahr hat 29 Februartage");

if (fehler === 0) {
  console.log("\n✅ Übersicht-Test bestanden.");
  process.exit(0);
} else {
  console.error(`\n❌ Übersicht-Test FEHLGESCHLAGEN: ${fehler} Problem(e).`);
  process.exit(1);
}
