// ============================================================
// Termine: sammelt ALLE Termine eines Projekts in ein gemeinsames,
// flaches Format – fuer die Kalender-Kachel auf der Übersicht.
//
// Vier Arten:
//   - foerderung: offizielle + eigene Fristen der gemerkten Förderungen
//   - intern:     interne Fristen des Projekts (ohne Förderung)
//   - team:       Fristen der ANDEREN Team-Geräte (aus dem Sync-Board)
//   - nachweis:   „Verwendungsnachweis fällig" je Geldquelle
//
// Jeder Termin sieht gleich aus:
//   { art, id, datum: "JJJJ-MM-TT", titel, untertitel, projektName, ziel }
// `ziel` sagt der Oberfläche, wohin ein Klick führt.
//
// WARUM ein Datum als Text "JJJJ-MM-TT" statt Date-Objekt: Ein Kalender
// ordnet Termine TAGEN zu. Mit Text gibt es keine Zeitzonen-Verschiebung
// (ein Date um Mitternacht kann sonst auf den Vortag rutschen).
//
// Reine Logik ohne Svelte – darum mit `npm run termine` testbar.
// ============================================================

import { fristNormalisieren, fristAlsDatum } from "./begriffe.js";

/// Anzeige-Namen der Termin-Arten (Reihenfolge = Legende).
export const TERMIN_ARTEN = {
  foerderung: "Förder-Frist",
  intern: "Interne Frist",
  team: "Team-Frist",
  nachweis: "Verwendungsnachweis",
};

const ISO_TAG_RE = /^\d{4}-\d{2}-\d{2}$/;

/// Date -> "JJJJ-MM-TT" (lokale Zeit).
export function isoTag(dt) {
  const m = String(dt.getMonth() + 1).padStart(2, "0");
  const t = String(dt.getDate()).padStart(2, "0");
  return `${dt.getFullYear()}-${m}-${t}`;
}

/// Beliebige Frist-Angabe (ISO, "MM-TT" jährlich, …) -> "JJJJ-MM-TT" oder null.
function alsTag(roh, heute) {
  const d = String(roh ?? "").trim();
  if (ISO_TAG_RE.test(d)) return d;
  const dt = fristAlsDatum(d, heute);
  return dt ? isoTag(dt) : null;
}

/// Tage von heute bis zum Tag (negativ = vorbei). Rechnet über Kalendertage
/// in UTC, damit Sommer-/Winterzeit nichts verfälscht.
export function tageBis(iso, heute = new Date()) {
  const [j, m, t] = iso.split("-").map(Number);
  const ziel = Date.UTC(j, m - 1, t);
  const jetzt = Date.UTC(heute.getFullYear(), heute.getMonth(), heute.getDate());
  return Math.round((ziel - jetzt) / 86400000);
}

/// Eigene Fristen kommen als Text ("JJJJ-MM-TT") oder als { datum, titel }.
function eigeneFrist(e) {
  if (typeof e === "string") return { datum: e, titel: "" };
  return { datum: e?.datum, titel: e?.titel || "" };
}

/// Sammelt alle Termine. Alle Angaben sind optional; fehlende Teile
/// ergeben einfach keine Termine dieser Art.
///   gemerkte        – Förderungs-Objekte der Merkliste
///   antraege        – { [foerderungId]: { offizielleFristen, eigeneFristen } }
///   interneFristen  – [{ id, datum, titel }]
///   quellen         – Geldquellen der Abrechnung [{ id, name, nachweisFrist }]
///   teamBoard       – Sync-Board-Einträge aller Team-Geräte
///   meineProjektIds – Board-IDs DIESES Geräts (werden nicht doppelt gezeigt)
///   foerderungLabel – Funktion: Board-Eintrag -> Anzeigename
///   heute           – für wiederkehrende Fristen ohne Jahr (Test-Hilfe)
/// Ergebnis ist nach Datum sortiert.
export function termineSammeln({
  gemerkte = [],
  antraege = {},
  interneFristen = [],
  quellen = [],
  teamBoard = null,
  meineProjektIds = [],
  foerderungLabel = null,
  heute = new Date(),
} = {}) {
  const liste = [];

  // 1. Förder-Fristen: JEDES konkrete Datum wird ein Termin (im Kalender
  //    sollen alle Einreichrunden sichtbar sein, nicht nur die nächste).
  for (const f of gemerkte) {
    if (!f) continue;
    const a = antraege[f.id];
    const offiziell = a?.offizielleFristen ?? f.fristen ?? [];
    for (const e of offiziell) {
      const datum = alsTag(fristNormalisieren(e).datum, heute);
      if (!datum) continue;
      liste.push({
        art: "foerderung", id: `f|${f.id}|of|${datum}`, datum,
        titel: f.name || "Förderung", untertitel: "", projektName: "",
        ziel: { bereich: "foerderung", foerderungId: f.id },
      });
    }
    for (const roh of a?.eigeneFristen ?? []) {
      const { datum: d, titel } = eigeneFrist(roh);
      const datum = alsTag(d, heute);
      if (!datum) continue;
      liste.push({
        art: "foerderung", id: `f|${f.id}|ef|${datum}|${titel}`, datum,
        titel: f.name || "Förderung", untertitel: titel || "Eigene Frist", projektName: "",
        ziel: { bereich: "foerderung", foerderungId: f.id },
      });
    }
  }

  // 2. Interne Fristen des Projekts.
  for (const t of interneFristen ?? []) {
    const datum = alsTag(t?.datum, heute);
    if (!datum) continue;
    liste.push({
      art: "intern", id: `i|${t.id ?? datum}`, datum,
      titel: t.titel || "Interne Frist", untertitel: "", projektName: "",
      ziel: { bereich: "fristen" },
    });
  }

  // 3. Verwendungsnachweis je Geldquelle. Bleibt lokal (sensible
  //    Abrechnungsdaten, Prinzip 1) – kommt nie aus dem Team-Board.
  for (const q of quellen ?? []) {
    const datum = alsTag(q?.nachweisFrist, heute);
    if (!datum) continue;
    liste.push({
      art: "nachweis", id: `n|${q.id}`, datum,
      titel: q.name || "Geldquelle", untertitel: "Verwendungsnachweis fällig", projektName: "",
      ziel: { bereich: "geldquellen", quelleId: q.id },
    });
  }

  // 4. Team-Fristen der ANDEREN Geräte (eigene kommen schon oben aus dem Tresor).
  for (const e of teamBoard ?? []) {
    if ((meineProjektIds ?? []).includes(e?.projekt_id)) continue;
    const projektName = e?.inhalt?.name || "Projekt";
    for (const f of e?.inhalt?.eintraege ?? []) {
      const fname = foerderungLabel ? foerderungLabel(f) : f.eigenesLabel || "Förderung";
      for (const d of f.offizielleFristen ?? []) {
        const datum = alsTag(fristNormalisieren(d).datum, heute);
        if (!datum) continue;
        liste.push({
          art: "team", id: `t|${e.projekt_id}|${f.foerderungId}|of|${datum}`, datum,
          titel: fname, untertitel: "", projektName, ziel: { bereich: "fristen" },
        });
      }
      for (const roh of f.eigeneFristen ?? []) {
        const { datum: d, titel } = eigeneFrist(roh);
        const datum = alsTag(d, heute);
        if (!datum) continue;
        liste.push({
          art: "team", id: `t|${e.projekt_id}|${f.foerderungId}|ef|${datum}|${titel}`, datum,
          titel: fname, untertitel: titel || "Eigene Frist", projektName, ziel: { bereich: "fristen" },
        });
      }
    }
    for (const t of e?.inhalt?.interneFristen ?? []) {
      const datum = alsTag(t?.datum, heute);
      if (!datum) continue;
      liste.push({
        art: "team", id: `t|${e.projekt_id}|int|${t.id || datum}`, datum,
        titel: t.titel || "Interne Frist", untertitel: "", projektName, ziel: { bereich: "fristen" },
      });
    }
  }

  return liste.sort((x, y) => x.datum.localeCompare(y.datum) || x.titel.localeCompare(y.titel));
}

/// Termine nach Tag gruppiert: { "JJJJ-MM-TT": [termin, …] } – für das Raster.
export function termineJeTag(termine) {
  const map = {};
  for (const t of termine) (map[t.datum] ??= []).push(t);
  return map;
}

/// Die nächsten `anzahl` Termine ab heute (heute zählt mit).
export function naechsteTermine(termine, anzahl = 5, heute = new Date()) {
  const ab = isoTag(heute);
  return termine.filter((t) => t.datum >= ab).slice(0, anzahl);
}
