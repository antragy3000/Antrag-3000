// Merged neue Förderer aus import/zusatz-foerderer.json in den App-Katalog
// src/lib/daten/foerderungen.json. Dedupe nach id UND normalisiertem Namen;
// validiert Region-/Stadt-Codes gegen orte.js. Aufruf:
//   node import/zusatz-anwenden.mjs
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { REGIONEN, STAEDTE } from "../src/lib/daten/orte.js";

const hier = path.dirname(fileURLToPath(import.meta.url));
const katalogPfad = path.join(hier, "..", "src", "lib", "daten", "foerderungen.json");
const zusatzPfad = path.join(hier, "zusatz-foerderer.json");

const katalog = JSON.parse(fs.readFileSync(katalogPfad, "utf8"));
const zusatz = JSON.parse(fs.readFileSync(zusatzPfad, "utf8"));

const norm = (s) => (s ?? "").toLowerCase().replace(/\s+/g, " ").trim();
const idSet = new Set(katalog.foerderungen.map((f) => f.id));
const nameSet = new Set(katalog.foerderungen.map((f) => norm(f.name)));

// Validierung der Region-/Stadt-Codes.
const regCodes = {};
for (const [land, list] of Object.entries(REGIONEN)) regCodes[land] = new Set(list.map((r) => r.code));
const staedte = new Set(STAEDTE.map((s) => s.name));
const probleme = [];
for (const f of zusatz.foerderungen) {
  const h = f.harte_kriterien ?? {};
  for (const key of ["wohnsitz_regionen", "durchfuehrungsort_regionen"]) {
    const laender = (key.startsWith("wohnsitz") ? h.wohnsitz : h.durchfuehrungsort) ?? [];
    for (const c of h[key] ?? []) {
      const ok = (laender.length ? laender : ["DE", "AT", "CH"]).some((l) => regCodes[l]?.has(c));
      if (!ok) probleme.push(`${f.id}: unbekannter Region-Code ${c}`);
    }
  }
  for (const key of ["wohnsitz_staedte", "durchfuehrungsort_staedte"]) {
    for (const s of h[key] ?? []) if (!staedte.has(s)) probleme.push(`${f.id}: Stadt unbekannt: ${s}`);
  }
  if (!f.id || !f.name) probleme.push(`Eintrag ohne id/name`);
}
if (probleme.length) {
  console.error("Validierung FEHLGESCHLAGEN:");
  probleme.forEach((p) => console.error("  !", p));
  process.exit(1);
}

let neu = 0, uebersprungen = 0;
for (const f of zusatz.foerderungen) {
  if (idSet.has(f.id) || nameSet.has(norm(f.name))) { uebersprungen++; continue; }
  katalog.foerderungen.push(f);
  idSet.add(f.id); nameSet.add(norm(f.name));
  neu++;
}

katalog.stand = new Date().toISOString().slice(0, 10);
fs.writeFileSync(katalogPfad, JSON.stringify(katalog, null, 2));
console.log(`Hinzugefügt: ${neu} · übersprungen (Dublette): ${uebersprungen} · Katalog jetzt: ${katalog.foerderungen.length}`);
