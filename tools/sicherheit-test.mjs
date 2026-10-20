// ============================================================
// Sicherheits-Test: beweist, dass die URL-Schleuse (sicherheit.js)
// gefaehrliche „Links" aus fremden Sync-Daten abweist und nur echte
// Web-/Mail-Links durchlaesst.
//
// HINTERGRUND: Synchronisierte Daten (geteilte Foerderer, Katalog vom
// Server) sind nicht vertrauenswuerdig. Bevor ein Link ans Betriebssystem
// zum Oeffnen geht (openUrl), muss er durch sichereWebUrl/sichereMailUrl.
// Dieser Test haelt diese Schleuse dauerhaft dicht: schluepft auch nur
// EIN gefaehrliches Schema durch, schlaegt er fehl (exit 1).
//
// Ausfuehren:  npm run sicherheit   (oder: node tools/sicherheit-test.mjs)
// ============================================================

import { readFileSync } from "node:fs";
import { sichereWebUrl, sichereMailUrl, kappen, macDownloadUrl, foerdererKontakt } from "../src/lib/sicherheit.js";
import {
  linkPruefen, dokumentPruefen, dokumentAnlegen, dokumentAendern, dokumentLoeschen,
  zuordnen, zuordnungLoesen, fuerFoerderung, nochNichtZugeordnet,
} from "../src/lib/arbeitsdokumente.js";
import { formularAlsText, textEinlesen, aenderungenAnwenden } from "../src/lib/cryptpadBruecke.js";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}

console.log("URL-Schleuse – gefaehrliche Schemata MUESSEN abgewiesen werden (null):");
// Genau diese Faelle waeren der Code-Injection-Weg: ein praeparierter
// „Webseiten"-Link, der beim Klick ein Programm/Schema startet.
const mussNull = [
  "file:///C:/Windows/System32/calc.exe",
  "file://server/share/evil.lnk",
  "javascript:alert(document.cookie)",
  "vbscript:msgbox(1)",
  "data:text/html,<script>alert(1)</script>",
  "ms-msdt:/id PCWDiagnostic",
  "search-ms:query=evil",
  "ms-settings:",
  "smb://angreifer/share",
  "JAVASCRIPT:alert(1)",
];
for (const u of mussNull) {
  pruefe(sichereWebUrl(u) === null, `abgewiesen: ${JSON.stringify(u)}`);
}

console.log("\nGrundregel: das Ergebnis ist IMMER null oder ein http/https-Link –");
console.log("nie etwas, das das Betriebssystem als Datei/Programm oeffnen wuerde:");
// Ein roher UNC-/Backslash-Pfad hat kein Schema und wird zu einem
// harmlosen https-Link entschaerft (kein file:/smb: mehr).
const mussSicher = [
  "\\\\angreifer\\share\\evil.lnk",
  "file:///C:/evil.exe",
  "ms-msdt:/id x",
  "https://ok.de",
  "www.ok.de",
];
for (const u of mussSicher) {
  const r = sichereWebUrl(u);
  pruefe(r === null || /^https?:\/\//.test(r), `sicheres Ergebnis fuer ${JSON.stringify(u)} → ${JSON.stringify(r)}`);
}

console.log("\nGewoehnliche Web-Links MUESSEN durchgelassen werden:");
pruefe(sichereWebUrl("https://kulturfoerderung.de") === "https://kulturfoerderung.de/",
  "https-Link bleibt");
pruefe(sichereWebUrl("http://example.org/foo?bar=1") === "http://example.org/foo?bar=1",
  "http-Link mit Query bleibt");
pruefe(sichereWebUrl("www.foo.de") === "https://www.foo.de/",
  "ohne Schema → https:// ergaenzt");
pruefe(sichereWebUrl("") === null, "leerer Link → null");
pruefe(sichereWebUrl(null) === null, "null → null");

console.log("\nMail-Schleuse:");
pruefe(sichereMailUrl("a@b.de") === "mailto:a%40b.de", "gueltige Adresse → kodiertes mailto");
pruefe(sichereMailUrl("a@b.de?cc=opfer@x.de") === null, "versteckte mailto-Felder → null");
pruefe(sichereMailUrl("a@b.de\nBcc: opfer@x.de") === null, "Zeilenumbruch (Header-Injection) → null");
pruefe(sichereMailUrl("kein-mail") === null, "keine Adresse → null");
pruefe(sichereMailUrl("") === null, "leer → null");

console.log("\nmacOS-Download – nur der eigene Update-Ort (Befund D-04):");
pruefe(
  macDownloadUrl("https://sync.antrag3000.de/updates/mac/Antrag-3000_0.4.18_universal.dmg") ===
    "https://sync.antrag3000.de/updates/mac/Antrag-3000_0.4.18_universal.dmg",
  "eigener Download-Ort bleibt"
);
const macMussNull = [
  "http://sync.antrag3000.de/updates/mac/a.dmg",
  "https://evil.example/updates/mac/a.dmg",
  "https://sync.antrag3000.de.evil.example/updates/mac/a.dmg",
  "https://sync.antrag3000.de@evil.example/updates/mac/a.dmg",
  "https://x@sync.antrag3000.de/updates/mac/a.dmg",
  "https://sync.antrag3000.de:8443/updates/mac/a.dmg",
  "https://sync.antrag3000.de/updates/mac/../../api/a.dmg",
  "https://sync.antrag3000.de/updates/mac/sub/a.dmg",
  "https://sync.antrag3000.de/updates/mac/a.dmg?x=1",
  "https://sync.antrag3000.de/updates/mac/a.dmg#x",
  "https://sync.antrag3000.de/updates/mac/a.pkg",
  "https://sync.antrag3000.de/updates/mac/",
  "javascript:alert(1)",
  "",
  null,
];
for (const u of macMussNull) {
  pruefe(macDownloadUrl(u) === null, `abgewiesen: ${JSON.stringify(u)}`);
}

console.log("\nText-Kappung:");
pruefe(kappen("x".repeat(1000), 200).length === 200, "lange Texte werden gekappt");
pruefe(kappen("ab\tcd") === "abcd", "Steuerzeichen (Tab) werden entfernt");
pruefe(kappen(null) === "", "null → leerer String");

// Kontaktperson laut Foerderer (Katalog-Feld `kontakt`, Fremddaten).
console.log("\nKontaktperson laut Foerderer:");
const kOk = foerdererKontakt({ kontakt: { name: "Dr. A. Berg", email: "berg@fonds.example", telefon: "030 123" } });
pruefe(kOk?.name === "Dr. A. Berg" && kOk?.email === "berg@fonds.example" && kOk?.telefon === "030 123", "gueltiger Kontakt bleibt erhalten");
pruefe(foerdererKontakt({}) === null, "ohne kontakt → null");
pruefe(foerdererKontakt({ kontakt: { name: " ", email: "", telefon: "" } }) === null, "nur Leerzeichen → null");
pruefe(foerdererKontakt({ kontakt: "Berg" }) === null, "kontakt als Text statt Objekt → null");
pruefe(foerdererKontakt({ kontakt: ["x"] }) === null, "kontakt als Liste → null");
for (const boese of [
  "javascript:alert(1)",
  "a@b.de?subject=x&body=y",
  "a@b.de,c@d.de",
  "a@b.de\nBcc: x@y.de",
  "<a@b.de>",
]) {
  pruefe(foerdererKontakt({ kontakt: { name: "X", email: boese } })?.email === "", `E-Mail verworfen: ${JSON.stringify(boese)}`);
}
pruefe(foerdererKontakt({ kontakt: { name: "x".repeat(500) } })?.name.length === 120, "Name auf 120 Zeichen gekappt");
pruefe(foerdererKontakt({ kontakt: { telefon: "1".repeat(200) } })?.telefon.length === 50, "Telefon auf 50 Zeichen gekappt");
pruefe(foerdererKontakt({ kontakt: { name: "A\u0007B" } })?.name === "AB", "Steuerzeichen im Namen entfernt");

// Arbeitsdokumente (Links zu gemeinsam geschriebenen Texten, z. B. CryptPad).
// Nur https-Links; die Zuordnung zu Foerderungen ist eine Referenz, keine Kopie.
console.log("\nArbeitsdokumente:");
const cp = "https://cryptpad.fr/pad/#/2/pad/edit/AbC-dEf+gHi/";
pruefe(linkPruefen(cp) === cp, "CryptPad-Link mit Schluessel im #-Teil bleibt unveraendert");
pruefe(linkPruefen("cryptpad.fr/pad/#/2/x") === "https://cryptpad.fr/pad/#/2/x", "Adresse ohne Schema bekommt https://");
for (const boese of ["http://cryptpad.fr/pad/", "javascript:alert(1)", "file:///C:/x", "data:text/html,x", "ftp://x.de", "", "https://x.de/" + "a".repeat(2100)]) {
  pruefe(linkPruefen(boese) === null, `Link abgewiesen: ${JSON.stringify(boese.slice(0, 40))}`);
}
pruefe(!dokumentPruefen("", cp).ok, "ohne Titel abgewiesen");
pruefe(dokumentPruefen("x".repeat(300), cp).titel?.length === 120, "Titel auf 120 Zeichen gekappt");
{
  const p = {};
  const d1 = dokumentAnlegen(p, "Projektbeschreibung", cp);
  const d2 = dokumentAnlegen(p, "Antragstext Musikfonds", cp, "f-musik");
  pruefe(p.arbeitsdokumente.length === 2 && d2.foerderungen[0] === "f-musik", "anlegen (mit und ohne Zuordnung)");
  zuordnen(p, d1.id, "f-musik"); zuordnen(p, d1.id, "f-musik");
  pruefe(d1.foerderungen.length === 1, "doppelte Zuordnung wird vermieden");
  pruefe(fuerFoerderung(p, "f-musik").length === 2 && nochNichtZugeordnet(p, "f-kultur").length === 2, "Auswahl je Foerderung stimmt");
  zuordnungLoesen(p, d1.id, "f-musik");
  pruefe(p.arbeitsdokumente.length === 2 && fuerFoerderung(p, "f-musik").length === 1, "Zuordnung loesen laesst das Dokument im Projekt");
  dokumentAendern(p, d2.id, "Neuer Titel", cp);
  pruefe(fuerFoerderung(p, "f-musik")[0].titel === "Neuer Titel", "Aenderung wirkt ueberall (Referenz, keine Kopie)");
  dokumentLoeschen(p, d2.id);
  pruefe(fuerFoerderung(p, "f-musik").length === 0 && p.arbeitsdokumente.length === 1, "Loeschen entfernt auch die Zuordnungen");
}

// CryptPad-Bruecke: Hin- und Rueckweg ueber die Zwischenablage. Rueckweg
// liest NUR markierte Abschnitte (kein freies Einlesen von Fliesstext).
console.log("\nCryptPad-Bruecke (Sammel-Formular):");
{
  const form = {
    projekttitel: "Klangraum", kurzbeschreibung: "Ein Festival.", beschreibung: "Absatz 1\n\nAbsatz 2",
    ziele: "", zeitraum: "Mai 2027", ort: "Basel", beteiligte: "Ensemble X",
    _eigeneFelder: [{ id: "e1", beschriftung: "Barrierefreiheit", typ: "textarea", wert: "Rampe" }, { id: "e2", beschriftung: " ", typ: "input", wert: "x" }],
  };
  const text = formularAlsText(form, "Klangraum");
  pruefe(text.includes("=== Projekttitel ===") && text.includes("=== Eigenes Feld: Barrierefreiheit ==="), "Export enthaelt Marker fuer Standard- und eigene Felder");
  pruefe(!text.includes("Eigenes Feld:  ==="), "eigene Felder ohne Namen werden nicht exportiert");
  const r0 = textEinlesen(text, form);
  pruefe(r0.aenderungen.length === 0 && r0.fehlend.length === 0 && r0.unveraendert === 8, "Rundreise ohne Aenderung: nichts zu uebernehmen");
  const bearbeitet = text.replace("Ein Festival.", "Ein viertaegiges Festival.\nMit Klangkunst.")
    .replace("=== Beteiligte / Mitwirkende / Partner*innen ===\nEnsemble X", "Beteiligte (Marker geloescht)\nEnsemble Y")
    .replace("Mai 2027", "Mai\n2027") + "\n=== Erfundenes Feld ===\nboese\n=== Projekttitel ===\nZweiter Titel\n";
  const r1 = textEinlesen(bearbeitet.replace(/\n/g, "\r\n"), form);
  const kurz = r1.aenderungen.find((a) => a.ziel.key === "kurzbeschreibung");
  pruefe(kurz?.neu === "Ein viertaegiges Festival.\nMit Klangkunst.", "geaenderter Abschnitt wird erkannt (auch mit CRLF)");
  // "Mai\n2027" in einem einzeiligen Feld wird zu "Mai 2027" = unveraendert.
  pruefe(!r1.aenderungen.some((a) => a.ziel.key === "zeitraum"), "einzeiliges Feld: Zeilenumbruch wird zu Leerzeichen");
  pruefe(r1.fehlend.includes("Beteiligte / Mitwirkende / Partner*innen"), "fehlender Marker wird gemeldet");
  pruefe(!r1.aenderungen.some((a) => a.ziel.key === "beteiligte"), "Abschnitt ohne Marker wird NICHT uebernommen");
  pruefe(r1.unbekannt.includes("Erfundenes Feld"), "unbekannter Marker wird gemeldet und ignoriert");
  pruefe(r1.doppelt.includes("Projekttitel") && !r1.aenderungen.some((a) => a.ziel.key === "projekttitel"), "doppelter Marker: nur das erste Vorkommen zaehlt");
  // Rich Text in CryptPad: Absaetze statt Zeilenumbrueche, geschuetzte Leerzeichen.
  const richText = text.replace(/\n/g, "\n\n").replace("Ein Festival.", "Ein Festival.");
  pruefe(textEinlesen(richText, form).aenderungen.length === 0, "nur Leerraum anders (Rich-Text-Kopie) = keine Aenderung");
  const r2 = textEinlesen("=== Kurzbeschreibung ===\nA\u0007B\u0000C\tD", form);
  pruefe(r2.aenderungen[0]?.neu === "ABC\tD", "Steuerzeichen raus, Tab bleibt");
  const kopie = structuredClone(form);
  aenderungenAnwenden(kopie, textEinlesen(text.replace("Rampe", "Rampe und Leitsystem"), form).aenderungen);
  pruefe(kopie._eigeneFelder[0].wert === "Rampe und Leitsystem" && kopie.projekttitel === "Klangraum", "Anwenden aendert nur das geaenderte Feld");
}

// CSP aller drei Apps (tauri.conf.json). Zweite Verteidigungslinie hinter der
// Schleuse oben: selbst wenn fremder Text doch als HTML im Fenster landet, darf
// er kein Skript ausfuehren und nichts nach aussen schicken. Inline-Skripte
// (Start-Skript von SvelteKit bzw. der <script>-Block in ui/index.html) erlaubt
// Tauri beim Bauen per Hash ('sha256-…'), dafuer braucht es kein
// 'unsafe-inline' (Befund D-11, Delta-Pentest 09/2026).
const erlaubtConnect = new Set(["'self'", "ipc:", "http://ipc.localhost"]);
for (const app of ["src-tauri", "admin-app/src-tauri", "foerderer-app/src-tauri"]) {
  console.log(`\nCSP ${app}:`);
  const conf = JSON.parse(readFileSync(new URL(`../${app}/tauri.conf.json`, import.meta.url), "utf8"));
  const csp = conf?.app?.security?.csp;
  pruefe(typeof csp === "string" && csp.length > 0, "CSP ist gesetzt (nicht null)");
  const direktiven = Object.fromEntries(
    String(csp ?? "")
      .split(";")
      .map((d) => d.trim().split(/\s+/))
      .filter((t) => t[0])
      .map(([name, ...quellen]) => [name, quellen]),
  );
  const skript = direktiven["script-src"] ?? direktiven["default-src"] ?? [];
  pruefe(skript.length > 0, "script-src (bzw. default-src) vorhanden");
  for (const verboten of ["'unsafe-inline'", "'unsafe-eval'", "*", "http:", "https:", "data:"]) {
    pruefe(!skript.includes(verboten), `script-src ohne ${verboten}`);
  }
  pruefe((direktiven["default-src"] ?? []).join(" ") === "'self'", "default-src nur 'self'");
  // Datensouveraenitaet: das Fenster selbst darf nur mit der App (IPC) reden;
  // Netzverkehr laeuft ausschliesslich ueber das Rust-Backend.
  const connect = direktiven["connect-src"] ?? [];
  pruefe(
    connect.length > 0 && connect.every((q) => erlaubtConnect.has(q)),
    `connect-src nur 'self'/ipc (ist: ${connect.join(" ") || "–"})`,
  );
}

// Admin-/Foerderer-Oberflaeche: KEIN <style>-Block in index.html. Tauri haengt
// an <style> beim Bauen eine Nonce; dann ignoriert die CSP 'unsafe-inline' und
// jede style="…"-Angabe der Oberflaeche wird geblockt (Layout kaputt). Die
// Gestaltung gehoert deshalb in ui/style.css.
console.log("\nOberflaechen ohne <style>-Block:");
for (const ui of ["admin-app/ui/index.html", "foerderer-app/ui/index.html"]) {
  const html = readFileSync(new URL(`../${ui}`, import.meta.url), "utf8");
  pruefe(!/<style[\s>]/i.test(html), `${ui} ohne <style> (Gestaltung in style.css)`);
}

if (fehler === 0) {
  console.log("\n✅ Sicherheits-Test bestanden: die URL-Schleuse haelt dicht.");
  process.exit(0);
} else {
  console.error(`\n❌ Sicherheits-Test FEHLGESCHLAGEN: ${fehler} Problem(e).`);
  process.exit(1);
}
