// ============================================================
// Aktiver Förder-Katalog (Phase 3 / Etappe 1).
//
// Der Katalog ist UNKRITISCH (Topf 2). Eine Standard-Fassung ist in der
// App eingebacken; ein Update kann sie zur Laufzeit ersetzen. Dies ist
// die EINZIGE Stelle, an der die App ihren Katalog hält – alle Bereiche
// (Alle Förderungen, Matching, Merkliste, KFP) lesen von hier, damit ein
// Update überall sofort wirkt.
// ============================================================

import standard from "./daten/foerderungen.json";

// Reaktiver Container. `daten` ist die aktive Katalog-Fassung,
// `quelle` zeigt, woher sie stammt ("standard" oder "datei").
export const katalog = $state({
  daten: standard,
  quelle: "standard",
});

/// Die mitgelieferte Standard-Fassung (Werkszustand).
export function standardKatalog() {
  return standard;
}

/// Prüft, ob ein eingelesenes Objekt ein brauchbarer Katalog ist:
/// gleiche Schema-Version wie die Standard-Fassung, eine Liste von
/// Förderungen, jede mit id und name. Gibt {ok, fehler} zurück.
export function pruefeKatalog(obj) {
  if (!obj || typeof obj !== "object") {
    return { ok: false, fehler: "Das ist keine Katalog-Datei." };
  }
  if (obj.schema_version !== standard.schema_version) {
    return {
      ok: false,
      fehler:
        "Diese Katalog-Version passt nicht zu dieser App-Version " +
        `(erwartet Schema ${standard.schema_version}, gefunden ${obj.schema_version ?? "?"}).`,
    };
  }
  if (!Array.isArray(obj.foerderungen)) {
    return { ok: false, fehler: "Im Katalog fehlt die Liste der Förderungen." };
  }
  for (const f of obj.foerderungen) {
    if (!f || typeof f !== "object" || !f.id || !f.name) {
      return { ok: false, fehler: "Mindestens eine Förderung hat keine id oder keinen Namen." };
    }
  }
  return { ok: true, fehler: null };
}

/// Setzt eine neue Katalog-Fassung als aktiv (nach erfolgreicher Prüfung).
export function setzeKatalog(neu, quelle = "datei") {
  katalog.daten = neu;
  katalog.quelle = quelle;
}

/// Zurück zur mitgelieferten Standard-Fassung.
export function setzeStandardKatalog() {
  katalog.daten = standard;
  katalog.quelle = "standard";
}

/// Vergleicht zwei Förderungs-Listen (alt vs. neu) anhand der id und
/// liefert die Änderungen für den Hinweis: was ist NEU, GEÄNDERT (gleiche
/// id, aber anderer Inhalt) oder ENTFERNT. Je Eintrag {id, name}.
export function vergleicheKataloge(altArr, neuArr) {
  const alt = new Map((altArr ?? []).map((f) => [f.id, f]));
  const neuM = new Map((neuArr ?? []).map((f) => [f.id, f]));
  const neu = [];
  const geaendert = [];
  for (const [id, f] of neuM) {
    if (!alt.has(id)) {
      neu.push({ id, name: f.name });
    } else if (JSON.stringify(alt.get(id)) !== JSON.stringify(f)) {
      geaendert.push({ id, name: f.name });
    }
  }
  const entfernt = [];
  for (const [id, f] of alt) {
    if (!neuM.has(id)) entfernt.push({ id, name: f.name });
  }
  return { neu, geaendert, entfernt };
}

/// Welche (obersten) Felder unterscheiden sich zwischen zwei Fassungen
/// derselben Förderung? Für die „NEU"-Markierung am konkret geänderten
/// Feld (z. B. foerderhoehe_text, fristen).
export function geaenderteFelder(alt, neu) {
  if (!alt || !neu) return [];
  const keys = new Set([...Object.keys(alt), ...Object.keys(neu)]);
  const out = [];
  for (const k of keys) {
    if (JSON.stringify(alt[k]) !== JSON.stringify(neu[k])) out.push(k);
  }
  return out;
}

// --- Bequeme Lese-Helfer (auch aus reinen .js-Modulen nutzbar) ---
export function foerderungen() {
  return katalog.daten.foerderungen ?? [];
}
export function katalogHinweis() {
  return katalog.daten.hinweis ?? "";
}
export function katalogStand() {
  return katalog.daten.stand ?? null;
}
