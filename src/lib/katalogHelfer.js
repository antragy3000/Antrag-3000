// Katalog-Helfer: reine Funktionen rund um den Förder-Katalog, die aus
// +page.svelte ausgelagert wurden, damit die Haupt-Datei kleiner bleibt.
// Verhalten unverändert.

import { kappen, sichereWebUrl } from "./sicherheit";
import { katalog } from "./katalog.svelte.js";

// Wandelt die vom Server geholten geteilten Förderer in Katalog-Form um
// (markiert mit `geteilt: true`). Robust gegen fehlende Felder, damit
// ein unvollständiger Datensatz die Anzeige nicht zerlegt.
export function teamFoerdererZuKatalog(roh) {
  return (roh ?? []).map((r) => {
    const i = r.inhalt ?? {};
    const hk = i.harte_kriterien ?? {};
    const wk = i.weiche_kriterien ?? {};
    // Diese Daten kommen von fremden Geräten: Texte kappen (gegen
    // aufgeblähte Datensätze + versteckte Steuerzeichen) und die
    // Webseite hier schon auf eine echte http/https-Adresse einschränken
    // (sichereWebUrl gibt sonst null → Feld bleibt leer).
    return {
      id: r.id,
      geteilt: true,
      name: kappen(i.name, 200) || "(ohne Name)",
      foerdergeber: kappen(i.foerdergeber, 200),
      land: i.land ?? "ANDERES",
      beschreibung: "", // bewusst leer – bleibt lokal beim Ersteller
      webseite: sichereWebUrl(i.webseite) ?? "",
      foerderhoehe_text: kappen(i.foerderhoehe_text, 200) || "—",
      fristen: Array.isArray(i.fristen) ? i.fristen : [],
      unvertraeglich_mit: Array.isArray(i.unvertraeglich_mit) ? i.unvertraeglich_mit : [],
      checkliste_vorschlag: Array.isArray(i.checkliste_vorschlag) ? i.checkliste_vorschlag : [],
      harte_kriterien: {
        wohnsitz: Array.isArray(hk.wohnsitz) ? hk.wohnsitz : [],
        durchfuehrungsort: Array.isArray(hk.durchfuehrungsort) ? hk.durchfuehrungsort : [],
        traegerschaft: Array.isArray(hk.traegerschaft) ? hk.traegerschaft : [],
        studentisch_erlaubt: hk.studentisch_erlaubt ?? true,
      },
      weiche_kriterien: {
        sparten: Array.isArray(wk.sparten) ? wk.sparten : [],
        projektarten: Array.isArray(wk.projektarten) ? wk.projektarten : [],
        budget_min: wk.budget_min ?? null,
        budget_max: wk.budget_max ?? null,
        waehrung: wk.waehrung ?? "EUR",
        zeitpunkt: wk.zeitpunkt ?? "fristen",
      },
    };
  });
}

// Klarname einer Förderung für die Team-Übersicht: bei eigener Förderung
// das mitgeschickte Label, sonst aus der (öffentlichen) Förder-Datenbank.
export function boardFoerderungLabel(eintrag) {
  if (eintrag?.eigenesLabel) return eintrag.eigenesLabel;
  const f = katalog.daten.foerderungen.find((x) => x.id === eintrag?.foerderungId);
  return f ? f.name : "Förderung";
}

// Pflegt je Projekt die „nicht mehr im Katalog"-Schattenkopien:
//  - entfernte, aber noch gemerkte/bearbeitete Förderungen als Ghost
//    sichern (damit Name + gespeicherter Status sichtbar bleiben),
//  - Ghosts wieder entfernen, sobald die Förderung zurück im Katalog ist.
// `daten` ist der (reaktive) Tresor-Zustand; wird von der aufrufenden
// Komponente übergeben, damit diese Funktion ohne globalen Zustand auskommt.
export function katalogGhostsAktualisieren(daten, altArr, neuArr) {
  const alt = new Map((altArr ?? []).map((f) => [f.id, f]));
  const neueIds = new Set((neuArr ?? []).map((f) => f.id));
  for (const projekt of daten.projekte ?? []) {
    const ghosts = Array.isArray(projekt.katalogGhosts) ? projekt.katalogGhosts : [];
    const referenziert = new Set([
      ...(projekt.merkliste ?? []),
      ...Object.keys(projekt.antraege ?? {}),
    ]);
    for (const [id, f] of alt) {
      if (neueIds.has(id)) continue;          // noch im Katalog
      if (!referenziert.has(id)) continue;    // nicht genutzt
      if ((projekt.eigeneFoerderungen ?? []).some((e) => e.id === id)) continue;
      if (ghosts.some((g) => g.id === id)) continue;
      ghosts.push({ ...f, nichtMehrImKatalog: true });
    }
    // Ghosts entfernen, deren Förderung wieder im Katalog ist.
    projekt.katalogGhosts = ghosts.filter((g) => !neueIds.has(g.id));
  }
}
