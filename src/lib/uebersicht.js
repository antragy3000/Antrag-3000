// ============================================================
// Übersicht: die Regeln hinter den Kacheln der Startseite.
//   - alsNaechstes:       was braucht jetzt Aufmerksamkeit?
//   - antraegeNachStatus: wie viele Anträge stehen wo?
//   - abrechnungStand:    Belege ohne Zuordnung, Soll/Zugeordnet je Quelle
//
// Reine Logik ohne Svelte – darum mit `npm run uebersicht` testbar.
// Liest nur Daten aus dem entsperrten Tresor, nichts verlässt das Gerät.
// ============================================================

import { tageBis } from "./termine.js";
import { ANTRAG_STATUS, ANTRAG_STANDARD } from "./status.js";

// Nur bei diesen Status ist eine Förder-Frist noch eine Aufgabe. Wer schon
// abgeschickt hat, wird nicht mehr daran erinnert.
const FRIST_NOCH_OFFEN = ["in_bearbeitung", "nachbessern"];

// Ab so vielen Tagen Restzeit gilt eine Frist als dringend (wie im Fristen-Bereich).
const DRINGEND_TAGE = 14;

// Interne Fristen und Nachweise kann man nicht „abhaken". Damit ein alter
// Termin nicht ewig als überfällig dasteht, verschwindet er nach dieser Zeit.
const UEBERFAELLIG_MAX_TAGE = 30;

/// Rangfolge: kleiner = weiter oben.
function rang(e) {
  if (e.tage !== null && e.tage < 0) return 0; // überfällig
  if (e.tage !== null && e.tage <= DRINGEND_TAGE) return 1; // bald fällig
  if (e.art === "status") return 2;
  if (e.art === "check") return 3;
  return 4; // Frist, aber noch Zeit
}

/// Alles, was jetzt Aufmerksamkeit braucht. Liefert { eintraege, gesamt }:
/// `eintraege` sind die obersten `anzahl`, `gesamt` zählt alle.
/// Team-Fristen fehlen bewusst: Das sind Aufgaben anderer Personen.
export function alsNaechstes({ termine = [], gemerkte = [], antraege = {}, anzahl = 5, heute = new Date() } = {}) {
  const liste = [];
  const schonGezeigt = new Set();

  for (const t of termine) {
    if (t.art === "team") continue;
    const tage = tageBis(t.datum, heute);
    if (t.art === "foerderung") {
      if (tage < 0) continue; // verpasste Einreichrunde ist keine Aufgabe mehr
      const status = antraege[t.ziel.foerderungId]?.status ?? ANTRAG_STANDARD;
      if (!FRIST_NOCH_OFFEN.includes(status)) continue;
      // Von mehreren offiziellen Runden zählt nur die nächste (Termine sind sortiert).
      if (!t.untertitel) {
        if (schonGezeigt.has(t.ziel.foerderungId)) continue;
        schonGezeigt.add(t.ziel.foerderungId);
      }
    } else if (tage < -UEBERFAELLIG_MAX_TAGE) {
      continue;
    }
    liste.push({
      id: t.id, art: "frist", terminArt: t.art,
      titel: t.titel, untertitel: t.untertitel, tage, ziel: t.ziel,
    });
  }

  for (const f of gemerkte) {
    if (!f) continue;
    const a = antraege[f.id];
    const ziel = { bereich: "foerderung", foerderungId: f.id };
    if (a?.status === "nachbessern") {
      liste.push({
        id: `s|${f.id}`, art: "status", titel: f.name || "Förderung",
        untertitel: "Antrag muss nachgebessert werden", tage: null, ziel,
      });
    }
    (a?.checkliste ?? []).forEach((p, i) => {
      if (p?.status !== "anfragen") return;
      liste.push({
        id: `c|${f.id}|${i}`, art: "check", titel: f.name || "Förderung",
        untertitel: `Anfragen: ${p.text || "Unterlage"}`, tage: null, ziel,
      });
    });
  }

  liste.sort((x, y) => rang(x) - rang(y) || (x.tage ?? 0) - (y.tage ?? 0));
  return { eintraege: liste.slice(0, anzahl), gesamt: liste.length };
}

/// Anzahl der gemerkten Anträge je Status (nur Status, die vorkommen, in der
/// festen Reihenfolge der Status-Kette). Noch nicht geöffnete Förderungen
/// haben keinen Antrag-Eintrag und zählen als Standard-Status.
export function antraegeNachStatus(gemerkte = [], antraege = {}) {
  const zaehler = new Map();
  for (const f of gemerkte) {
    if (!f) continue;
    const key = antraege[f.id]?.status ?? ANTRAG_STANDARD;
    zaehler.set(key, (zaehler.get(key) ?? 0) + 1);
  }
  return ANTRAG_STATUS.filter((s) => zaehler.has(s.key)).map((s) => ({
    key: s.key,
    label: s.key === "anderer" ? "anderer Status" : s.label,
    farbe: s.farbe,
    anzahl: zaehler.get(s.key),
  }));
}

/// Stand der Abrechnung. `sichtbar` ist false, solange es weder Belege noch
/// Geldquellen gibt – dann bleibt die Kachel weg.
/// `rechner` = { belegFrei, quelleSoll, zugeordnetJeQuelle } aus abrechnung.js.
/// WARUM übergeben statt importieren: abrechnung.js zieht über kfp.js den ganzen
/// Katalog mit und liesse sich im Test ausserhalb der App nicht laden.
export function abrechnungStand(abrechnung, { belegFrei, quelleSoll, zugeordnetJeQuelle }) {
  const belege = abrechnung?.belege ?? [];
  const quellen = abrechnung?.quellen ?? [];
  const zu = zugeordnetJeQuelle(belege);
  return {
    sichtbar: belege.length > 0 || quellen.length > 0,
    belegeOffen: belege.filter((b) => belegFrei(b) > 0.005).length,
    quellen: quellen.map((q) => ({
      id: q.id,
      name: q.name || "Geldquelle",
      soll: quelleSoll(q),
      zugeordnet: zu.get(q.id) ?? 0,
    })),
  };
}

/// "in 5 Tagen" / "heute" / "seit 2 Tagen überfällig".
export function tageText(tage) {
  if (tage === 0) return "heute";
  if (tage === 1) return "morgen";
  if (tage > 1) return `in ${tage} Tagen`;
  return tage === -1 ? "seit 1 Tag überfällig" : `seit ${-tage} Tagen überfällig`;
}

/// Die Tage eines Monats als Wochen-Raster (Montag zuerst). Tage ausserhalb
/// des Monats sind null. `monat` ist 0-basiert (wie bei Date).
export function monatsRaster(jahr, monat) {
  const tageImMonat = new Date(jahr, monat + 1, 0).getDate();
  const versatz = (new Date(jahr, monat, 1).getDay() + 6) % 7; // Mo=0 … So=6
  const zellen = Array(versatz).fill(null);
  const mm = String(monat + 1).padStart(2, "0");
  for (let t = 1; t <= tageImMonat; t++) {
    zellen.push({ tag: t, iso: `${jahr}-${mm}-${String(t).padStart(2, "0")}` });
  }
  while (zellen.length % 7) zellen.push(null);
  return zellen;
}
