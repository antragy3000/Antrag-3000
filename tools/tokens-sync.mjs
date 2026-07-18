#!/usr/bin/env node
/* ============================================================
   tokens-sync – kopiert die gemeinsamen Design-Tokens aus
   design/tokens.css in jede Oberflaeche.

   Warum: Die vier Programme werden getrennt ausgeliefert (App
   gebaut, zwei Tauri-Apps mit eigenem ui-Ordner, Website per
   FTP). Sie koennen zur Laufzeit keine gemeinsame Datei teilen.
   Darum gibt es EINE Quelle (design/tokens.css) und dieses
   Skript kopiert ihren :root-Block in einen markierten Bereich
   jeder Datei – so bleibt jede Oberflaeche eigenstaendig.

   Aufruf:
     node tools/tokens-sync.mjs           -> schreibt (Sync)
     node tools/tokens-sync.mjs --check   -> nur pruefen (CI/Waechter),
                                             Exit 1 bei Abweichung

   In jeder Ziel-Datei muss der Bereich markiert sein:
     / * >>> TOKENS ... * /
       ... (wird ersetzt) ...
     / * <<< TOKENS * /
   ============================================================ */

import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const wurzel = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const quelle = path.join(wurzel, "design", "tokens.css");

// Alle Oberflaechen, die die Tokens brauchen.
const ziele = [
  "src/theme.css",
  "foerderer-app/ui/index.html",
  "admin-app/ui/index.html",
  "website/index.html",
  "website/impressum.html",
  "website/datenschutz.html",
  "website/code-signing.html",
];

const START = "/* >>> TOKENS";
const ENDE = "/* <<< TOKENS */";

const nurPruefen = process.argv.includes("--check");

/** Holt den Inhalt zwischen erster { und letzter } aus der Quelle. */
function deklarationen(css) {
  const a = css.indexOf("{");
  const b = css.lastIndexOf("}");
  if (a < 0 || b < 0) throw new Error("design/tokens.css: kein :root{ … } gefunden");
  return css
    .slice(a + 1, b)
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((z) => z.replace(/\s+$/, ""))
    .join("\n")
    .trim();
}

/** Baut den fertigen Block mit der Einrueckung der START-Zeile. */
function baueBlock(decls, einzug) {
  const inner = decls
    .split("\n")
    .map((z) => (z.trim() === "" ? "" : einzug + "  " + z.trim()))
    .join("\n");
  return (
    `${einzug}/* >>> TOKENS (generiert von: npm run tokens · Quelle: design/tokens.css · NICHT von Hand aendern) */\n` +
    `${einzug}:root{\n${inner}\n${einzug}}\n` +
    `${einzug}${ENDE}`
  );
}

const tokenDecls = deklarationen(await readFile(quelle, "utf8"));

let abweichungen = 0;
let fehlendeMarker = 0;

for (const rel of ziele) {
  const datei = path.join(wurzel, rel);
  let text;
  try {
    text = await readFile(datei, "utf8");
  } catch {
    console.warn(`  übersprungen (nicht gefunden): ${rel}`);
    continue;
  }

  const sIdx = text.indexOf(START);
  const eIdx = text.indexOf(ENDE, sIdx + 1);
  if (sIdx < 0 || eIdx < 0) {
    console.warn(`  ⚠ Marker fehlen in ${rel} – bitte einmal einfügen:\n` +
      `      /* >>> TOKENS */\n      /* <<< TOKENS */`);
    fehlendeMarker++;
    continue;
  }

  // Einrückung der START-Zeile ermitteln.
  const zeilenAnfang = text.lastIndexOf("\n", sIdx) + 1;
  const einzug = text.slice(zeilenAnfang, sIdx).match(/^[ \t]*/)[0];

  const block = baueBlock(tokenDecls, einzug);
  const vorher = text.slice(zeilenAnfang, eIdx + ENDE.length);
  const neu = text.slice(0, zeilenAnfang) + block + text.slice(eIdx + ENDE.length);

  if (vorher.replace(/\r\n/g, "\n") === block) {
    console.log(`  = aktuell: ${rel}`);
    continue;
  }

  abweichungen++;
  if (nurPruefen) {
    console.error(`  ✗ veraltet: ${rel}`);
  } else {
    await writeFile(datei, neu);
    console.log(`  ✓ aktualisiert: ${rel}`);
  }
}

if (fehlendeMarker > 0 && nurPruefen) process.exitCode = 1;

if (nurPruefen) {
  if (abweichungen > 0) {
    console.error(`\n${abweichungen} Datei(en) nicht synchron. Bitte  npm run tokens  ausführen.`);
    process.exitCode = 1;
  } else {
    console.log("\nAlle Oberflächen sind synchron mit design/tokens.css.");
  }
} else {
  console.log(`\nFertig. Tokens aus design/tokens.css in die Oberflächen kopiert.`);
}
