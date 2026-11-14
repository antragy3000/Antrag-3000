// ============================================================
// Organisationen (Variante B+, Etappe O2)
//
// Jede Organisation ist ein eigener Tresor (eigene Datei, eigenes oder
// geteiltes Passwort, siehe src-tauri/src/tresor.rs). Diese Datei kennt nur
// die reine Logik dazu, ohne Tauri, damit sie testbar ist:
//
//  - welcher Name eine Organisation anzeigt,
//  - die Liste für den Umschalter (offene und gesperrte Organisationen),
//  - welche Namen jeder Tresor über die ANDEREN kennt,
//  - wie eine neue Organisation ihre Daten bekommt.
//
// Namen gesperrter Organisationen: Rust kennt nur IDs (Zufallszeichen) und
// verrät keine Namen. Damit der Umschalter trotzdem „Kulturverein Blau
// (gesperrt)" zeigen kann, merkt sich jeder Tresor die Namen der anderen
// in `bekannteOrganisationen` – verschlüsselt, wie alles im Tresor.
//
// Ordner: Jede Organisation hat unter Dokumente\Antrag 3000 einen eigenen Ordner
// (organisation.ordner), darin liegen ihre Projektordner. Solange der einmalige
// Umzug alter Projektordner offen ist (organisation.ordnerUmzug), liegen sie
// noch direkt in der Wurzel; das Frontend gibt dann "" an Rust weiter.
//
// Bewusst ohne Imports: tresorFormat.js nutzt orgFelderErgaenzen, und der
// Test (tools/organisationen-test.mjs) läuft so direkt in Node.
// ============================================================

/** Ersatzname für eine gesperrte Organisation, deren Namen kein offener Tresor kennt. */
export const NAME_UNBEKANNT = "Gesperrte Organisation";

/**
 * Ergänzt in älteren Tresoren die Felder der Organisationen (für
 * normalisieren). Liefert true, wenn etwas ergänzt wurde.
 * @param {any} d
 */
export function orgFelderErgaenzen(d) {
  let veraendert = false;
  if (!d.organisation || typeof d.organisation !== "object" || typeof d.organisation.name !== "string") {
    d.organisation = { name: "" };
    veraendert = true;
  }
  if (!Array.isArray(d.bekannteOrganisationen)) {
    d.bekannteOrganisationen = [];
    veraendert = true;
  }
  return veraendert;
}

/**
 * Der Ordner, in dem die Dateien dieser Organisation gerade liegen.
 * "" = direkt in Dokumente\Antrag 3000 (Umzug noch offen).
 * @param {any} daten
 */
export function orgOrdnerVon(daten) {
  const org = daten?.organisation;
  if (!org || org.ordnerUmzug) return "";
  return typeof org.ordner === "string" ? org.ordner : "";
}

/**
 * Der Name, den eine Organisation im Umschalter zeigt. Eigener Name zuerst;
 * sonst das, was in den Stammdaten steht (ältere Tresore haben noch keinen
 * Namen).
 * @param {any} daten
 */
export function orgName(daten) {
  const eigen = String(daten?.organisation?.name ?? "").trim();
  if (eigen) return eigen;
  const person = daten?.stammdaten?.person ?? {};
  const org = String(person.organisation ?? "").trim();
  if (org) return org;
  const kuenstler = String(person.kuenstlername ?? "").trim();
  if (kuenstler) return kuenstler;
  const voll = `${person.vorname ?? ""} ${person.nachname ?? ""}`.trim();
  return voll || "Meine Organisation";
}

/**
 * Liste für den Umschalter: alle Organisationen, die es auf diesem Gerät gibt
 * (`alleIds` von Rust), offene mit ihrem Namen, gesperrte mit dem Namen, den
 * ein offener Tresor von ihnen kennt. Sortiert nach Name.
 * @param {{ id: string, daten: any }[]} offene
 * @param {string[]} alleIds
 * @param {string | null} aktivId
 * @returns {{ id: string, name: string, offen: boolean, aktiv: boolean }[]}
 */
export function orgListe(offene, alleIds, aktivId) {
  const bekannt = new Map();
  for (const o of offene) {
    for (const b of o.daten?.bekannteOrganisationen ?? []) {
      if (b?.id && b?.name && !bekannt.has(b.id)) bekannt.set(b.id, b.name);
    }
  }
  const offenNach = new Map(offene.map((o) => [o.id, o]));
  const ids = [...new Set([...alleIds, ...offene.map((o) => o.id)])];
  return ids
    .map((id) => {
      const o = offenNach.get(id);
      return {
        id,
        name: o ? orgName(o.daten) : bekannt.get(id) ?? NAME_UNBEKANNT,
        offen: !!o,
        aktiv: id === aktivId,
      };
    })
    .sort((a, b) => a.name.localeCompare(b.name, "de") || a.id.localeCompare(b.id));
}

/**
 * Bringt in jedem offenen Tresor die Liste `bekannteOrganisationen` auf den
 * neuesten Stand: offene Organisationen mit ihrem aktuellen Namen, gesperrte
 * mit dem zuletzt bekannten, verschwundene (nicht mehr in `alleIds`) fliegen
 * raus. Liefert die IDs der Tresore, deren Liste sich geändert hat – nur die
 * müssen gespeichert werden.
 * @param {{ id: string, daten: any }[]} offene
 * @param {string[]} alleIds
 * @returns {string[]}
 */
export function bekannteAbgleichen(offene, alleIds) {
  const liste = orgListe(offene, alleIds, null).filter((e) => e.name !== NAME_UNBEKANNT || e.offen);
  const geaendert = [];
  for (const o of offene) {
    const neu = liste.filter((e) => e.id !== o.id).map((e) => ({ id: e.id, name: e.name }));
    const alt = o.daten.bekannteOrganisationen ?? [];
    if (JSON.stringify(alt) !== JSON.stringify(neu)) {
      o.daten.bekannteOrganisationen = neu;
      geaendert.push(o.id);
    }
  }
  return geaendert;
}

/**
 * Prüft einen Namen für eine neue oder umbenannte Organisation.
 * @param {string} name
 * @param {{ id: string, name: string }[]} liste  aus orgListe
 * @param {string | null} eigeneId  beim Umbenennen die eigene ID
 * @returns {{ ok: boolean, name: string, fehler: string }}
 */
export function namePruefen(name, liste, eigeneId = null) {
  const sauber = String(name ?? "").trim().replace(/\s+/g, " ");
  if (!sauber) return { ok: false, name: sauber, fehler: "" };
  if (sauber.length > 80) return { ok: false, name: sauber, fehler: "Der Name ist zu lang (höchstens 80 Zeichen)." };
  const schon = liste.find((e) => e.id !== eigeneId && e.name.toLocaleLowerCase("de") === sauber.toLocaleLowerCase("de"));
  if (schon) return { ok: false, name: sauber, fehler: `Es gibt schon eine Organisation „${schon.name}“.` };
  return { ok: true, name: sauber, fehler: "" };
}

/**
 * Daten für eine neue Organisation: `frisch` (= frischerTresor()) mit Namen. Mit
 * übernommen werden nur Einstellungen, die zum GERÄT gehören (Netz-Wahl,
 * Server, Stand der Förder-Datenbank), damit die neue Organisation nicht
 * erneut fragt. Stammdaten, Projekte und Team-Anbindung bleiben leer – eine
 * Organisation ist ja gerade ein anderer Absender.
 * @param {any} frisch   ein frischer Tresor (frischerTresor())
 * @param {any} vorlage  Daten der gerade offenen Organisation (oder null)
 * @param {string} name
 */
export function neueOrgDaten(frisch, vorlage, name) {
  const d = frisch;
  d.organisation = { name };
  d.bekannteOrganisationen = [];
  if (vorlage) {
    for (const feld of ["netzModus", "einzelServer", "modusHinweisAus"]) {
      if (vorlage[feld] !== undefined) d[feld] = vorlage[feld];
    }
    for (const feld of ["katalogStand", "katalogFeldDiff"]) {
      if (vorlage[feld] && typeof vorlage[feld] === "object") d[feld] = structuredClone(vorlage[feld]);
    }
  }
  return d;
}

/**
 * Welche Organisation nach dem Entsperren aktiv wird: die zuletzt benutzte
 * (Zeitstempel `zuletztGeoeffnet` im Tresor), sonst die erste nach Name.
 * @param {{ id: string, daten: any }[]} offene
 */
export function startOrg(offene) {
  if (!offene.length) return null;
  const mitZeit = offene.filter((o) => typeof o.daten?.zuletztGeoeffnet === "string");
  if (mitZeit.length) {
    return mitZeit.reduce((a, b) => (b.daten.zuletztGeoeffnet > a.daten.zuletztGeoeffnet ? b : a)).id;
  }
  return orgListe(offene, [], null)[0].id;
}
