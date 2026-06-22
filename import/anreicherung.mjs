// Wendet recherchierte Anreicherungen (anreicherung.json) auf den
// importierten Katalog an. Pro id werden NUR die angegebenen Felder
// gesetzt/überschrieben; der Rest bleibt. Idempotent – beliebig oft
// ausführbar. Aufruf:  node import/anreicherung.mjs
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const hier = path.dirname(fileURLToPath(import.meta.url));
const katalogPfad = path.join(hier, "foerderungen-import.json");
const patchPfad = path.join(hier, "anreicherung.json");

const katalog = JSON.parse(fs.readFileSync(katalogPfad, "utf8"));
const patches = JSON.parse(fs.readFileSync(patchPfad, "utf8"));

const byId = new Map(katalog.foerderungen.map((f) => [f.id, f]));
let geaendert = 0;
const fehlend = [];

for (const [id, p] of Object.entries(patches)) {
  const f = byId.get(id);
  if (!f) { fehlend.push(id); continue; }
  if (p.beschreibung) f.beschreibung = p.beschreibung;
  if (p.webseite) f.webseite = p.webseite;
  if (p.foerderhoehe_text) f.foerderhoehe_text = p.foerderhoehe_text;
  if (Array.isArray(p.fristen)) f.fristen = p.fristen;
  if (Array.isArray(p.sparten)) f.weiche_kriterien.sparten = p.sparten;
  if (Array.isArray(p.projektarten)) f.weiche_kriterien.projektarten = p.projektarten;
  if (p.zeitpunkt) f.weiche_kriterien.zeitpunkt = p.zeitpunkt;
  if (typeof p.budget_min !== "undefined") f.weiche_kriterien.budget_min = p.budget_min;
  if (typeof p.budget_max !== "undefined") f.weiche_kriterien.budget_max = p.budget_max;
  if (Array.isArray(p.wohnsitz)) f.harte_kriterien.wohnsitz = p.wohnsitz;
  if (Array.isArray(p.durchfuehrungsort)) f.harte_kriterien.durchfuehrungsort = p.durchfuehrungsort;
  if (Array.isArray(p.durchfuehrungsort_regionen)) f.harte_kriterien.durchfuehrungsort_regionen = p.durchfuehrungsort_regionen;
  if (Array.isArray(p.wohnsitz_regionen)) f.harte_kriterien.wohnsitz_regionen = p.wohnsitz_regionen;
  if (Array.isArray(p.checkliste_vorschlag)) f.checkliste_vorschlag = p.checkliste_vorschlag;
  if (p.recherchiert) f.recherchiert = true;
  geaendert++;
}

katalog.stand = new Date().toISOString().slice(0, 10);
fs.writeFileSync(katalogPfad, JSON.stringify(katalog, null, 2));

const recherchiert = katalog.foerderungen.filter((f) => f.recherchiert).length;
console.log(`Angereichert: ${geaendert} Patches angewendet.`);
console.log(`Recherchiert markiert: ${recherchiert} / ${katalog.foerderungen.length}`);
if (fehlend.length) console.log("WARNUNG: id nicht gefunden:", fehlend.join(", "));
