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

// ------------------------------------------------------------------
// Organisation weitergeben (O6c)
//
// Weitergegeben wird nur der INHALT einer Organisation – per Positivliste.
// Alles andere (Team-Verbindung mit Ausweis, Geräte-Einstellungen, noch nicht
// gesendete Meldungen, Liste der anderen Organisationen) bleibt auf dem Gerät.
// Ein neues Feld im Tresor wird damit von selbst NICHT weitergegeben, bis es
// hier bewusst eingetragen ist (organisationen-test.mjs prüft, dass jedes Feld
// von frischerTresor() in genau einer der beiden Listen steht).
// ------------------------------------------------------------------

export const WEITERGABE_FORMAT = "antrag3000-organisation";
/** Felder, die mitgehen (dazu der Name aus `organisation`). */
export const WEITERGABE_FELDER = ["stammdaten", "projekte", "projektgruppen", "aktivesProjektId"];
/** Felder, die bewusst auf dem Gerät bleiben (der Empfänger hat eigene). */
export const NICHT_WEITERGEGEBEN = [
  "version",
  "modus",
  "modusHinweisAus",
  "netzModus",
  "einzelServer",
  "sync",
  "teamCa",
  "katalogMeldungen",
  "katalogStand",
  "katalogFeldDiff",
  "bekannteOrganisationen",
  "organisation", // nur der Name geht mit, nie der Ordner
];

/**
 * Was in die Weitergabe-Datei kommt (als JSON). `d` = die Daten der
 * Organisation (ein Schnappschuss, wird nicht verändert).
 * @param {any} d
 */
export function weitergabeInhalt(d) {
  /** @type {Record<string, any>} */
  const aus = { format: WEITERGABE_FORMAT, version: 1, organisation: { name: orgName(d) } };
  for (const feld of WEITERGABE_FELDER) {
    if (d?.[feld] !== undefined) aus[feld] = structuredClone(d[feld]);
  }
  return aus;
}

/** @param {unknown} x @returns {x is Record<string, any>} */
const istObjekt = (x) => !!x && typeof x === "object" && !Array.isArray(x);

/**
 * Der vorgeschlagene Name aus einer Weitergabe (fremde Daten, darum geprüft).
 * @param {any} roh
 */
export function weitergabeName(roh) {
  const n = istObjekt(roh) && istObjekt(roh.organisation) ? roh.organisation.name : "";
  return typeof n === "string" ? n.trim().replace(/\s+/g, " ").slice(0, 80) : "";
}

/**
 * Ein freier Name: `name`, sonst „name (2)“, „name (3)“ …
 * @param {string} name
 * @param {{ id: string, name: string }[]} liste
 */
export function freierOrgName(name, liste) {
  const basis = name || "Organisation";
  for (let i = 1; i < 1000; i++) {
    const kandidat = i === 1 ? basis : `${basis} (${i})`;
    if (namePruefen(kandidat, liste).ok) return kandidat;
  }
  return basis;
}

/**
 * Baut aus einer Weitergabe (`roh`, fremde Daten!) die Daten einer NEUEN
 * Organisation: Geräte-Einstellungen wie bei „Neue Organisation“ aus der
 * gerade offenen (`vorlage`), vom Inhalt nur die Felder der Positivliste,
 * einzeln zugewiesen. Prüft vorher den Aufbau – lieber eine klare Meldung als
 * eine halb übernommene Organisation. normalisieren() folgt beim Aufrufer.
 * @param {any} frisch   frischerTresor()
 * @param {any} vorlage  Daten der offenen Organisation (oder null)
 * @param {any} roh      JSON.parse(Inhalt der Datei)
 * @param {string} name  der (geprüfte) Name der neuen Organisation
 * @returns {{ ok: true, daten: any } | { ok: false, fehler: string }}
 */
export function weitergabeUebernehmen(frisch, vorlage, roh, name) {
  const kaputt = { ok: /** @type {const} */ (false), fehler: "Die Datei enthält keine lesbare Organisation." };
  if (!istObjekt(roh) || roh.format !== WEITERGABE_FORMAT) return kaputt;
  if (roh.version !== 1) {
    return { ok: false, fehler: "Diese Datei stammt aus einer neueren Programmversion. Bitte aktualisiere Antrag 3000." };
  }
  if (roh.stammdaten !== undefined && !istObjekt(roh.stammdaten)) return kaputt;
  if (!Array.isArray(roh.projekte)) return kaputt;
  const ids = new Set();
  for (const p of roh.projekte) {
    if (!istObjekt(p) || typeof p.id !== "string" || !p.id || typeof p.name !== "string" || ids.has(p.id)) return kaputt;
    ids.add(p.id);
  }
  if (roh.projektgruppen !== undefined && (!Array.isArray(roh.projektgruppen) || !roh.projektgruppen.every(istObjekt))) {
    return kaputt;
  }
  const aktiv = roh.aktivesProjektId;
  if (aktiv !== undefined && aktiv !== null && typeof aktiv !== "string") return kaputt;

  const d = neueOrgDaten(frisch, vorlage, name);
  if (roh.stammdaten) {
    d.stammdaten = roh.stammdaten;
    // Das Logo wird als <img src> angezeigt. Aus fremder Hand nur ein
    // eingebettetes Bild – eine Internet-Adresse würde beim Anzeigen still
    // abgerufen (Prinzip 1: keine Verbindung ohne Zutun).
    const logo = d.stammdaten.logo;
    if (logo !== undefined && !(typeof logo === "string" && /^data:image\/(png|jpeg|gif|webp);base64,/i.test(logo))) {
      d.stammdaten.logo = "";
    }
  }
  d.projekte = roh.projekte;
  d.projektgruppen = roh.projektgruppen ?? [];
  d.aktivesProjektId = aktiv ?? null;
  return { ok: true, daten: d };
}

// --- Aufbau einer übernommenen Organisation prüfen -------------------------
// Eine präparierte .a3korganisation kann Werte der falschen Art einschmuggeln
// (Zahl statt Text, null in einer Liste …). normalisieren() ergänzt nur, was
// FEHLT; die Anzeige stürzt dann an solchen Stellen ab (Einbruchtest O7: z. B.
// Kostenplan, Teile, Belege, Projekt-Menü). Darum wird die Organisation NACH
// normalisieren() gegen diesen Bauplan geprüft und bei einer Abweichung ganz
// abgelehnt – nichts wird still „repariert“ oder weggelassen.
//
// Der Bauplan verlangt nur, was eine Datei aus der App selbst immer erfüllt:
// Listen sind Listen, Einträge haben die Art, die die App anlegt, und Kennungen
// und Namen sind Text. Unbekannte zusätzliche Felder stören nicht (offene
// Objekte); die meisten Felder sind optional, geprüft wird dann nur ihre Art.

const TEXT = "text";
const ZAHL_ODER_TEXT = "zahl-oder-text";
const JA_NEIN = "ja-nein";
/** @param {any} art */ const liste = (art) => ({ liste: art });
/** @param {any} art */ const oderNull = (art) => ({ oderNull: art });
/** @param {Record<string, any>} felder @param {string[]} [pflicht] */
const objekt = (felder, pflicht = []) => ({ objekt: felder, pflicht });
/** Objekt mit beliebigen Schlüsseln (z. B. Anträge je Förder-ID). @param {any} art */
const zuordnung = (art) => ({ zuordnung: art });
/** Eine der genannten Arten. @param {...any} arten */
const eines = (...arten) => ({ eines: arten });

const DATEI_VERWEIS = objekt({ name: TEXT, ref: TEXT });
// Frist einer Förderung: "JJJJ-MM-TT"/"MM-TT" oder – aus Katalog und Team –
// { datum, hinweis } (siehe fristNormalisieren in begriffe.js, sync.js).
const FRIST = eines(TEXT, objekt({ datum: TEXT, hinweis: TEXT }, ["datum"]));
const FOERDERUNG = objekt(
  {
    id: TEXT, name: TEXT, foerdergeber: TEXT, beschreibung: TEXT, webseite: TEXT, land: TEXT,
    einreich_url: TEXT, frist_hinweis: TEXT, foerderhoehe_text: TEXT,
    fristen: liste(FRIST), unvertraeglich_mit: liste(TEXT), checkliste_vorschlag: liste(TEXT),
    harte_kriterien: objekt({ wohnsitz: liste(TEXT), durchfuehrungsort: liste(TEXT), traegerschaft: liste(TEXT) }),
    weiche_kriterien: objekt({ sparten: liste(TEXT), projektarten: liste(TEXT) }, ["sparten", "projektarten"]),
  },
  ["id", "name", "weiche_kriterien"],
);
const PROJEKT = objekt(
  {
    id: TEXT, name: TEXT, erstellt: TEXT, ablageUrl: TEXT,
    gruppeId: oderNull(TEXT), vorgaengerId: oderNull(TEXT),
    archiviert: JA_NEIN, kfpHinweisAusblenden: JA_NEIN, cryptpadHinweisAus: JA_NEIN,
    bausteine: liste(objekt({ id: TEXT, name: TEXT, beschreibung: TEXT, ort: TEXT, zeitraum: TEXT }, ["id", "name"])),
    fragebogen: oderNull(objekt({ sparten: liste(TEXT), projektarten: liste(TEXT) })),
    merkliste: liste(TEXT),
    formular: objekt({ _eigeneFelder: liste(objekt({ id: TEXT, titel: TEXT, text: TEXT }, ["id"])) }),
    kfp: objekt(
      {
        kosten: liste(objekt({ name: TEXT, posten: liste(objekt({ id: TEXT, bezeichnung: TEXT, erlaeuterung: TEXT, betrag: ZAHL_ODER_TEXT, bausteinId: oderNull(TEXT) })) }, ["posten"])),
        finanzierung: liste(objekt({ name: TEXT, posten: liste(objekt({ foerderId: oderNull(TEXT), bezeichnung: TEXT, betrag: ZAHL_ODER_TEXT, anteile: zuordnung(ZAHL_ODER_TEXT) })) }, ["posten"])),
      },
      ["kosten", "finanzierung"],
    ),
    antraege: zuordnung(objekt({
      status: TEXT, statusFrei: TEXT,
      abdeckung: oderNull(liste(TEXT)),
      offizielleFristen: liste(TEXT),
      eigeneFristen: liste(objekt({ datum: TEXT, titel: TEXT })),
      kontakt: objekt({ ansprechpartner: TEXT, email: TEXT, telefon: TEXT, notiz: TEXT }),
      checkliste: liste(objekt({ text: TEXT, status: TEXT, statusFrei: TEXT, datei: TEXT })),
    })),
    eigeneFoerderungen: liste(FOERDERUNG),
    katalogGhosts: liste(FOERDERUNG),
    katalogAktualisiert: liste(TEXT),
    interneFristen: liste(objekt({ id: TEXT, datum: TEXT, titel: TEXT }, ["id"])),
    abrechnung: objekt(
      {
        sachbericht: TEXT,
        belege: liste(objekt({
          id: TEXT, nr: ZAHL_ODER_TEXT, datum: TEXT, empfaenger: TEXT, zweck: TEXT, brutto: ZAHL_ODER_TEXT,
          mwst_satz: ZAHL_ODER_TEXT, zahlungsart: TEXT, kostenstelle: oderNull(TEXT), status: TEXT, notiz: TEXT,
          dateien: liste(DATEI_VERWEIS), zuordnungen: liste(objekt({ quelleId: TEXT, betrag: ZAHL_ODER_TEXT })),
        }, ["id"])),
        quellen: liste(objekt({
          id: TEXT, typ: TEXT, foerderId: oderNull(TEXT), name: TEXT, soll: ZAHL_ODER_TEXT, nachweisFrist: TEXT,
          sachbericht: TEXT, dateien: liste(DATEI_VERWEIS),
        }, ["id"])),
      },
      ["belege", "quellen"],
    ),
    arbeitsdokumente: liste(objekt({ id: TEXT, titel: TEXT, url: TEXT, foerderungen: liste(TEXT) }, ["id"])),
  },
  ["id", "name", "merkliste", "formular", "kfp", "antraege", "abrechnung"],
);
const PROJEKTGRUPPE = objekt({ id: TEXT, name: TEXT }, ["id", "name"]);

/**
 * Liefert den Pfad der ersten Abweichung vom Bauplan oder null.
 * @param {any} wert @param {any} art @param {string} pfad
 * @returns {string | null}
 */
function abweichung(wert, art, pfad) {
  if (art === TEXT) return typeof wert === "string" ? null : pfad;
  if (art === JA_NEIN) return typeof wert === "boolean" ? null : pfad;
  if (art === ZAHL_ODER_TEXT) return typeof wert === "string" || (typeof wert === "number" && Number.isFinite(wert)) ? null : pfad;
  if (art.oderNull) return wert === null ? null : abweichung(wert, art.oderNull, pfad);
  if (art.eines) return art.eines.some((/** @type {any} */ a) => abweichung(wert, a, pfad) === null) ? null : pfad;
  if (art.liste) {
    if (!Array.isArray(wert)) return pfad;
    for (let i = 0; i < wert.length; i++) {
      const a = abweichung(wert[i], art.liste, `${pfad}.${i}`);
      if (a) return a;
    }
    return null;
  }
  if (art.zuordnung) {
    if (!istObjekt(wert)) return pfad;
    for (const [k, v] of Object.entries(wert)) {
      const a = abweichung(v, art.zuordnung, `${pfad}.${k}`);
      if (a) return a;
    }
    return null;
  }
  if (!istObjekt(wert)) return pfad;
  for (const k of art.pflicht) if (wert[k] === undefined) return `${pfad}.${k}`;
  for (const [k, unterArt] of Object.entries(art.objekt)) {
    if (wert[k] === undefined) continue;
    const a = abweichung(wert[k], unterArt, `${pfad}.${k}`);
    if (a) return a;
  }
  return null;
}

/**
 * Prüft eine übernommene Organisation (nach normalisieren()) gegen den Bauplan.
 * @param {any} d
 * @returns {{ ok: true } | { ok: false, fehler: string, stelle: string }}
 */
export function weitergabeAufbauPruefen(d) {
  const stelle =
    abweichung(d?.projekte, liste(PROJEKT), "projekte") ??
    abweichung(d?.projektgruppen, liste(PROJEKTGRUPPE), "projektgruppen") ??
    abweichung(d?.aktivesProjektId, oderNull(TEXT), "aktivesProjektId");
  if (stelle) {
    return { ok: false, fehler: "Die Datei enthält keine lesbare Organisation (Aufbau beschädigt).", stelle };
  }
  return { ok: true };
}

/** ID der ersten Organisation – auf JEDEM Gerät dieselbe (siehe tresor.rs). */
export const ERSTE_ORG = "0000000000000000";

/**
 * Vorschlag je Organisation, bevor eine Sicherung eingespielt wird.
 * `orgs` kommt aus sicherung_pruefen ({id, vorhanden, daten|null}), `lokale`
 * sind die Organisationen auf diesem Gerät ({id, name}, wie orgListe).
 *
 * Gleiche ID heißt gleiche Organisation – mit EINER Ausnahme: Die erste
 * Organisation hat auf jedem Gerät dieselbe ID. Eine Sicherung vom alten
 * Computer darf nicht still eine fremde ersetzen; dort wird nur bei gleichem
 * Namen „ersetzen" vorgeschlagen, sonst „als Kopie dazu" (andere: true).
 * Jede andere ID ist zufällig und damit eindeutig: „ersetzen", auch wenn die
 * Organisation seither umbenannt wurde oder ein eigenes Passwort hat.
 * @param {{ id: string, vorhanden: boolean, daten: string | null }[]} orgs
 * @param {{ id: string, name: string }[]} lokale
 * @returns {{ id: string, name: string | null, vorhanden: boolean, lokalName: string | null, wahl: string, andere: boolean }[]}
 */
export function einspielVorschlag(orgs, lokale) {
  const lokal = new Map((lokale ?? []).map((e) => [e.id, e.name]));
  return (orgs ?? []).map((o) => {
    /** @type {any} */
    let d = null;
    try {
      d = o.daten ? JSON.parse(o.daten) : null;
    } catch {
      d = null;
    }
    const name = d ? orgName(d) : null;
    const lokalName = o.vorhanden ? lokal.get(o.id) ?? null : null;
    const andere = !!o.vorhanden && o.id === ERSTE_ORG && !(name && name === lokalName);
    const wahl = !o.vorhanden ? "dazu" : andere ? "kopie" : "ersetzen";
    return { id: o.id, name, vorhanden: !!o.vorhanden, lokalName, wahl, andere };
  });
}

/**
 * Wo die Daten einer OFFENEN Organisation gerade liegen – für Ergebnisse aus
 * dem Netz, die erst ankommen, nachdem man womöglich umgeschaltet hat (Team
 * verbinden, Ausweis erneuern). Wer vor dem Warten die ID festhält und hier
 * erst beim Schreiben nachschlägt, trifft immer die richtige Organisation:
 * die aktive, eine andere offene – oder keine mehr (gesperrt/entfernt), dann
 * wird das Ergebnis verworfen.
 * @param {string | null} id        die vorher festgehaltene ID
 * @param {string | null} aktivId   die jetzt aktive Organisation
 * @param {any} aktivDaten          ihre Daten
 * @param {{ id: string, daten: any }[]} andere  die übrigen offenen
 * @returns {{ daten: any, aktiv: boolean } | null}
 */
export function offeneOrgFinden(id, aktivId, aktivDaten, andere) {
  if (!id) return null;
  if (id === aktivId) return aktivDaten ? { daten: aktivDaten, aktiv: true } : null;
  const o = (andere ?? []).find((x) => x.id === id);
  return o ? { daten: o.daten, aktiv: false } : null;
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
