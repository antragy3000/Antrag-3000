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

import { sichereWebUrl, sichereMailUrl, kappen } from "../src/lib/sicherheit.js";

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

console.log("\nText-Kappung:");
pruefe(kappen("x".repeat(1000), 200).length === 200, "lange Texte werden gekappt");
pruefe(kappen("ab\tcd") === "abcd", "Steuerzeichen (Tab) werden entfernt");
pruefe(kappen(null) === "", "null → leerer String");

if (fehler === 0) {
  console.log("\n✅ Sicherheits-Test bestanden: die URL-Schleuse haelt dicht.");
  process.exit(0);
} else {
  console.error(`\n❌ Sicherheits-Test FEHLGESCHLAGEN: ${fehler} Problem(e).`);
  process.exit(1);
}
