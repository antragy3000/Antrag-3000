// ============================================================
// Projektgruppen-Test: prüft die Umstellung von Programmreihen auf
// Projektgruppen und die Regeln für Gruppen – ohne die App zu starten.
//
// Ausfuehren:  npm run projektgruppen   (oder: node tools/projektgruppen-test.mjs)
// ============================================================

import {
  gruppenUmstellen,
  leereGruppenEntfernen,
  projekteNachGruppen,
  gruppeAnlegen,
  gruppenNamePruefen,
  gruppeAufloesen,
} from "../src/lib/projektgruppen.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

console.log("Umstellung einer alten Sicherung mit Reihen:");
const alt = {
  reihen: [
    { id: "r1", name: "Festival" },
    { id: "r2", name: "Ohne Projekte" },
  ],
  projekte: [
    { id: "a", name: "Festival 2025", reiheId: "r1", archiviert: true },
    { id: "b", name: "Festival 2026", reiheId: "r1", archiviert: false },
    { id: "c", name: "Einzeln", reiheId: null, archiviert: false },
    { id: "d", name: "Verwaist", reiheId: "gibt-es-nicht", archiviert: false },
    { id: "e", name: "Ganz alt", archiviert: false },
  ],
};
pruefe(gruppenUmstellen(alt) === true, "meldet eine Änderung, damit neu gespeichert wird");
pruefe(alt.projektgruppen.length === 1 && alt.projektgruppen[0].name === "Festival", "Reihe wird zur Gruppe, Name bleibt");
pruefe(alt.projektgruppen[0].id === "r1", "Kennung bleibt gleich");
pruefe(!("reihen" in alt), "die alte Liste ist weg");
pruefe(alt.projekte.every((p) => !("reiheId" in p)), "das alte Feld ist bei allen Projekten weg");
pruefe(alt.projekte[0].gruppeId === "r1" && alt.projekte[1].gruppeId === "r1", "aktive und archivierte Projekte bleiben in der Gruppe");
pruefe(alt.projekte[2].gruppeId === null, "Projekt ohne Reihe bleibt ohne Gruppe");
pruefe(alt.projekte[3].gruppeId === null, "Verweis auf eine fehlende Reihe fällt auf keine Gruppe zurück");
pruefe(alt.projekte[4].gruppeId === null, "sehr alter Stand ohne das Feld bekommt keine Gruppe");
pruefe(gruppenUmstellen(alt) === false, "zweites Öffnen ändert nichts mehr");

console.log("Umstellung, Sonderfälle:");
const frisch = { projekte: [] };
gruppenUmstellen(frisch);
pruefe(Array.isArray(frisch.projektgruppen) && frisch.projektgruppen.length === 0, "Stand ganz ohne Reihen bekommt eine leere Liste");

// Zwischendurch mit der alten Version geöffnet: Beide Listen sind da.
const gemischt = {
  projektgruppen: [{ id: "g1", name: "Neu" }],
  reihen: [{ id: "r1", name: "Alt" }, { id: "g1", name: "Doppelt" }],
  projekte: [
    { id: "a", name: "A", gruppeId: "g1", reiheId: "r1" },
    { id: "b", name: "B", reiheId: "r1" },
  ],
};
gruppenUmstellen(gemischt);
pruefe(gemischt.projekte[0].gruppeId === "g1", "vorhandene Gruppe gewinnt gegen die alte Reihe");
pruefe(gemischt.projekte[1].gruppeId === "r1", "Projekt nur mit Reihe kommt in deren Gruppe");
pruefe(gemischt.projektgruppen.find((g) => g.id === "g1").name === "Neu", "gleiche Kennung überschreibt keine Gruppe");
pruefe(gemischt.projektgruppen.length === 2, "keine Gruppe geht verloren oder doppelt sich");

// Die alte Version ergänzt beim Öffnen leere Reihen und reiheId: null.
const zurueck = {
  projektgruppen: [{ id: "g1", name: "Festival" }],
  reihen: [],
  projekte: [{ id: "a", name: "A", gruppeId: "g1", reiheId: null }],
};
gruppenUmstellen(zurueck);
pruefe(zurueck.projekte[0].gruppeId === "g1" && zurueck.projektgruppen.length === 1, "Gruppen überstehen einen Abstecher in die alte Version");

const kaputt = {
  reihen: [null, { name: "ohne id" }, { id: "r1" }],
  projekte: [{ id: "a", name: "A", reiheId: "r1" }],
};
gruppenUmstellen(kaputt);
pruefe(kaputt.projektgruppen.length === 0 && kaputt.projekte[0].gruppeId === null, "kaputte Einträge fallen weg, das Projekt bleibt");
const keineListe = { reihen: "unsinn", projekte: [] };
gruppenUmstellen(keineListe);
pruefe(!("reihen" in keineListe) && keineListe.projektgruppen.length === 0, "unbrauchbare Reihen-Liste stört nicht");

console.log("Leere Gruppen:");
const leer = {
  projektgruppen: [{ id: "g1", name: "Voll" }, { id: "g2", name: "Archiv" }, { id: "g3", name: "Leer" }],
  projekte: [
    { id: "a", gruppeId: "g1", archiviert: false },
    { id: "b", gruppeId: "g2", archiviert: true },
  ],
};
pruefe(leereGruppenEntfernen(leer) === true, "meldet das Entfernen");
pruefe(leer.projektgruppen.map((g) => g.id).join() === "g1,g2", "leere Gruppe verschwindet, Gruppe nur mit Archiv-Projekten bleibt");
pruefe(leereGruppenEntfernen(leer) === false, "ohne leere Gruppe ändert sich nichts");

console.log("Reihenfolge im Menü:");
const gruppen = [{ id: "g1", name: "Eins" }, { id: "g2", name: "Zwei" }, { id: "g3", name: "Drei" }];
const a = { id: "a", gruppeId: null };
const b = { id: "b", gruppeId: "g2" };
const c = { id: "c", gruppeId: "g1" };
const liste = projekteNachGruppen([a, b, c], gruppen);
pruefe(liste.map((e) => e.gruppe?.name ?? "ohne").join() === "Eins,Zwei,ohne", "Gruppen in ihrer Reihenfolge, Projekte ohne Gruppe zuletzt");
pruefe(!liste.some((e) => e.gruppe?.id === "g3"), "Gruppe ohne Projekt in dieser Liste fehlt");
pruefe(projekteNachGruppen([], gruppen).length === 0, "leere Liste bleibt leer");
const verwaist = projekteNachGruppen([{ id: "x", gruppeId: "weg" }], gruppen);
pruefe(verwaist.length === 1 && verwaist[0].gruppe === null, "Projekt verschwindet nie, auch bei unbekannter Gruppe");

console.log("Anlegen und Namen:");
const anlegen = { projektgruppen: [{ id: "g1", name: "Festival" }], projekte: [] };
pruefe(gruppeAnlegen(anlegen, "festival ", () => "x") === "g1", "gleicher Name führt zur bestehenden Gruppe");
pruefe(anlegen.projektgruppen.length === 1, "dabei entsteht keine zweite Gruppe");
pruefe(gruppeAnlegen(anlegen, "   ", () => "x") === null, "leerer Name legt nichts an");
pruefe(gruppeAnlegen(anlegen, " Werkschau ", () => "g2") === "g2", "neuer Name liefert die neue Kennung");
pruefe(anlegen.projektgruppen[1].id === "g2" && anlegen.projektgruppen[1].name === "Werkschau", "neuer Name legt eine Gruppe an");

const namen = [{ id: "g1", name: "Festival" }, { id: "g2", name: "Werkschau" }];
pruefe(gruppenNamePruefen("Neu", namen, "g1").ok, "freier Name ist erlaubt");
pruefe(gruppenNamePruefen("FESTIVAL", namen, "g1").ok, "eigene Gruppe darf die Schreibweise ändern");
const belegt = gruppenNamePruefen(" werkschau", namen, "g1");
pruefe(!belegt.ok && belegt.fehler.includes("Werkschau"), "Name einer anderen Gruppe wird mit Meldung abgelehnt");
const leerName = gruppenNamePruefen("  ", namen, "g1");
pruefe(!leerName.ok && leerName.fehler === "", "leere Eingabe: nicht erlaubt, aber ohne Meldung");

console.log("Auflösen:");
const aufloesen = {
  projektgruppen: [{ id: "g1", name: "Festival" }, { id: "g2", name: "Andere" }],
  projekte: [
    { id: "a", gruppeId: "g1", archiviert: false },
    { id: "b", gruppeId: "g1", archiviert: true },
    { id: "c", gruppeId: "g2", archiviert: false },
  ],
};
gruppeAufloesen(aufloesen, "g1");
pruefe(aufloesen.projektgruppen.map((g) => g.id).join() === "g2", "nur diese Gruppe ist weg");
pruefe(aufloesen.projekte.length === 3, "alle Projekte bleiben erhalten");
pruefe(aufloesen.projekte[0].gruppeId === null && aufloesen.projekte[1].gruppeId === null, "aktive und archivierte Projekte sind ohne Gruppe");
pruefe(aufloesen.projekte[2].gruppeId === "g2", "Projekte anderer Gruppen bleiben, wo sie sind");

if (fehler) {
  console.error(`\n✗ ${fehler} Prüfung(en) fehlgeschlagen.`);
  process.exit(1);
}
console.log("\n✓ Projektgruppen OK: Reihen werden umgestellt, kein Projekt geht verloren.");
