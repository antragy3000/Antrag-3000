// Baut die Sammler-Rohquelle server/katalog/rohquelle.json aus dem
// kanonischen App-Katalog src/lib/daten/foerderungen.json.
//
// HINTERGRUND (in einfachen Worten):
// Der Sammler auf dem Server (`server sammeln <datei>`) vergleicht eine
// "Rohquelle" mit dem aktuell VERÖFFENTLICHTEN Katalog in der Server-DB
// und legt aus den Unterschieden Vorschläge an (neu / geändert). Der Admin
// gibt sie in der Admin-App frei – erst dann landen sie im Live-Katalog,
// den die Nutzer-Apps ziehen.
//
// Diese Rohquelle ist also der FEED unserer Recherche. Sobald wir hier im
// Repo neue Förderer recherchiert haben (z. B. über import/zusatz-...),
// erzeugen wir damit die Rohquelle und legen sie auf die NAS in den
// Ordner server/katalog/. Der wöchentliche Sammler-Lauf macht daraus dann
// automatisch Freigabe-Vorschläge – ohne dass wir je den ganzen Katalog
// von Hand hochladen müssen.
//
// Aufruf:
//   node import/rohquelle-bauen.mjs
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { REGIONEN, STAEDTE } from "../src/lib/daten/orte.js";

const hier = path.dirname(fileURLToPath(import.meta.url));
const katalogPfad = path.join(hier, "..", "src", "lib", "daten", "foerderungen.json");
const zielPfad = path.join(hier, "..", "server", "katalog", "rohquelle.json");

const katalog = JSON.parse(fs.readFileSync(katalogPfad, "utf8"));
const liste = katalog.foerderungen ?? [];

// --- Validierung der Region-/Stadt-Codes gegen orte.js (wie im Merge) ---
const regCodes = {};
for (const [land, l] of Object.entries(REGIONEN)) regCodes[land] = new Set(l.map((r) => r.code));
const staedte = new Set(STAEDTE.map((s) => s.name));
const probleme = [];
const ids = new Set();
for (const f of liste) {
  if (!f.id || !f.name) probleme.push("Eintrag ohne id/name");
  if (f.id && ids.has(f.id)) probleme.push(`doppelte id: ${f.id}`);
  if (f.id) ids.add(f.id);
  const h = f.harte_kriterien ?? {};
  for (const key of ["wohnsitz_regionen", "durchfuehrungsort_regionen"]) {
    const laender = (key.startsWith("wohnsitz") ? h.wohnsitz : h.durchfuehrungsort) ?? [];
    for (const c of h[key] ?? []) {
      const ok = (laender.length ? laender : ["DE", "AT", "CH"]).some((l) => regCodes[l]?.has(c));
      if (!ok) probleme.push(`${f.id}: unbekannter Region-Code ${c} (${key})`);
    }
  }
  for (const key of ["wohnsitz_staedte", "durchfuehrungsort_staedte"]) {
    for (const s of h[key] ?? []) if (!staedte.has(s)) probleme.push(`${f.id}: Stadt unbekannt: ${s}`);
  }
}
if (probleme.length) {
  console.error("Validierung FEHLGESCHLAGEN – Rohquelle NICHT geschrieben:");
  probleme.forEach((p) => console.error("  !", p));
  process.exit(1);
}

const rohquelle = {
  _hinweis:
    "Sammler-Rohquelle (server sammeln server/katalog/rohquelle.json). Automatisch erzeugt aus src/lib/daten/foerderungen.json via import/rohquelle-bauen.mjs – nicht von Hand bearbeiten. Der Sammler vergleicht diese Liste mit dem veröffentlichten Katalog und legt Vorschläge an, die der Admin freigibt.",
  schema_version: katalog.schema_version ?? 1,
  erzeugt_am: new Date().toISOString().slice(0, 10),
  foerderungen: liste,
};
fs.writeFileSync(zielPfad, JSON.stringify(rohquelle, null, 2));
console.log(`Rohquelle geschrieben: ${path.relative(path.join(hier, ".."), zielPfad)} · ${liste.length} Förderer`);
