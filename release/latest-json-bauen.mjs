// Erzeugt server/updates/latest.json aus den signierten Build-Artefakten
// (Phase 3 / Etappe 5). So muss niemand das Manifest von Hand bauen.
//
// Voraussetzung: Es wurde signiert gebaut (npm run tauri build mit gesetzter
// TAURI_SIGNING_PRIVATE_KEY-Umgebungsvariable), sodass im Ordner
//   src-tauri/target/release/bundle/nsis/
// ein "...-setup.exe" UND die zugehoerige "...-setup.exe.sig" liegen.
//
// Aufruf (die Release-Notiz wird der Nutzerin im Update-Dialog angezeigt):
//   node release/latest-json-bauen.mjs "Was ist neu in dieser Version"
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const hier = path.dirname(fileURLToPath(import.meta.url));
const wurzel = path.join(hier, "..");

// Update-Adresse der NAS (muss zum Endpoint in tauri.conf.json passen).
const ENDPOINT_BASIS = "http://100.78.49.51:8445/updates";

const conf = JSON.parse(
  fs.readFileSync(path.join(wurzel, "src-tauri", "tauri.conf.json"), "utf8"),
);
const version = conf.version;
const notes = process.argv[2] ?? `Version ${version}`;

const nsisDir = path.join(wurzel, "src-tauri", "target", "release", "bundle", "nsis");
if (!fs.existsSync(nsisDir)) {
  console.error(`FEHLER: Bundle-Ordner fehlt:\n  ${nsisDir}`);
  console.error("Zuerst signiert bauen:  npm run tauri build");
  process.exit(1);
}
const dateien = fs.readdirSync(nsisDir);
// WICHTIG: die Datei zur AKTUELLEN Version waehlen. Liegen im Bundle-Ordner
// noch aeltere "...-setup.exe" (von frueheren Builds), wuerde ein blindes
// find() evtl. die falsche Version ins Manifest schreiben.
const setup =
  dateien.find((d) => d.includes(version) && d.endsWith("-setup.exe")) ??
  dateien.find((d) => d.endsWith("-setup.exe"));
const sig = setup && dateien.includes(setup + ".sig") ? setup + ".sig" : undefined;
if (!setup || !sig) {
  console.error("FEHLER: Setup-.exe oder .sig nicht gefunden in:");
  console.error(`  ${nsisDir}`);
  console.error("Wurde signiert gebaut (Umgebungsvariable TAURI_SIGNING_PRIVATE_KEY gesetzt)?");
  process.exit(1);
}

const signature = fs.readFileSync(path.join(nsisDir, sig), "utf8").trim();

const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    // Leerzeichen im Dateinamen werden fuer die URL kodiert (%20).
    "windows-x86_64": { signature, url: `${ENDPOINT_BASIS}/${encodeURIComponent(setup)}` },
  },
};

const ziel = path.join(wurzel, "server", "updates", "latest.json");
fs.writeFileSync(ziel, JSON.stringify(manifest, null, 2));

console.log(`latest.json geschrieben (Version ${version}).`);
console.log("");
console.log("Jetzt auf die NAS in den Ordner  updates/  hochladen:");
console.log(`  1. ${setup}`);
console.log(`  2. server/updates/latest.json`);
console.log("(Die .sig-Datei NICHT hochladen – ihr Inhalt steckt im latest.json.)");
