// ============================================================
// Projektgruppen: ordnen Projekte im Menü unter einem gemeinsamen Namen.
//
// Eine Gruppe ist nur eine Überschrift: Der Ordner eines Projekts, seine
// Anträge und Belege hängen nicht daran. Gruppen liegen im Tresor und
// gehen nie ins Team-Board (sync.js zählt sie nicht auf).
//
// Hieß früher „Programmreihe" (`reihen`, `reiheId`). Ältere Tresore und
// Sicherungen stellt gruppenUmstellen() beim Öffnen um.
//
// Reine Logik ohne Svelte – darum mit `npm run projektgruppen` testbar.
// ============================================================

/// Bringt einen Tresor auf den Stand mit Projektgruppen: übernimmt alte Reihen,
/// entfernt Verweise auf nicht mehr vorhandene Gruppen und leere Gruppen.
/// Liefert true, wenn etwas geändert wurde (dann neu speichern).
/** @param {any} d */
export function gruppenUmstellen(d) {
  let veraendert = false;
  if (!Array.isArray(d.projektgruppen)) {
    d.projektgruppen = [];
    veraendert = true;
  }
  if ("reihen" in d) {
    const vorhanden = new Set(d.projektgruppen.map((/** @type {any} */ g) => g?.id));
    for (const r of Array.isArray(d.reihen) ? d.reihen : []) {
      if (r?.id && typeof r.name === "string" && !vorhanden.has(r.id)) {
        d.projektgruppen.push({ id: r.id, name: r.name });
      }
    }
    delete d.reihen;
    veraendert = true;
  }
  const ids = new Set(d.projektgruppen.map((/** @type {any} */ g) => g?.id));
  for (const p of d.projekte ?? []) {
    if ("reiheId" in p) {
      if (!p.gruppeId) p.gruppeId = p.reiheId ?? null;
      delete p.reiheId;
      veraendert = true;
    }
    // Ein Verweis auf eine fehlende Gruppe fällt auf „keine Gruppe" zurück.
    if (p.gruppeId === undefined || (p.gruppeId !== null && !ids.has(p.gruppeId))) {
      p.gruppeId = null;
      veraendert = true;
    }
  }
  if (leereGruppenEntfernen(d)) veraendert = true;
  return veraendert;
}

/// Entfernt Gruppen ohne Projekt. Archivierte Projekte zählen mit, damit ihre
/// Gruppe im Archiv erhalten bleibt. Liefert true, wenn etwas entfernt wurde.
/** @param {any} d */
export function leereGruppenEntfernen(d) {
  const genutzt = new Set((d.projekte ?? []).map((/** @type {any} */ p) => p.gruppeId));
  const bleiben = d.projektgruppen.filter((/** @type {any} */ g) => genutzt.has(g?.id));
  if (bleiben.length === d.projektgruppen.length) return false;
  d.projektgruppen = bleiben;
  return true;
}

/// Ordnet Projekte für das Menü: erst die Gruppen in ihrer Reihenfolge, danach
/// die Projekte ohne Gruppe. Gruppen ohne Projekt in dieser Liste fehlen.
/// Liefert [{ gruppe: {id,name}|null, projekte: […] }].
/**
 * @param {any[]} projekte
 * @param {any[]} gruppen
 */
export function projekteNachGruppen(projekte, gruppen) {
  const liste = [];
  const ids = new Set();
  for (const g of gruppen ?? []) {
    ids.add(g.id);
    const ps = projekte.filter((p) => p.gruppeId === g.id);
    if (ps.length) liste.push({ gruppe: g, projekte: ps });
  }
  const ohne = projekte.filter((p) => !ids.has(p.gruppeId));
  if (ohne.length) liste.push({ gruppe: null, projekte: ohne });
  return liste;
}

/// Legt eine Gruppe an oder findet eine gleichnamige und liefert ihre id.
/// Speichert NICHT selbst – der Aufrufer speichert.
/**
 * @param {any} d
 * @param {any} name
 * @param {() => string} neueId
 */
export function gruppeAnlegen(d, name, neueId) {
  const sauber = String(name ?? "").trim();
  if (!sauber) return null;
  const schon = d.projektgruppen.find(
    (/** @type {any} */ g) => g.name.toLowerCase() === sauber.toLowerCase()
  );
  if (schon) return schon.id;
  const id = neueId();
  d.projektgruppen.push({ id, name: sauber });
  return id;
}

/// Prüft den neuen Namen einer Gruppe, damit im Menü nicht zwei Gruppen gleich
/// heißen. Liefert { ok, name, fehler }; bei leerer Eingabe ohne Meldung.
/**
 * @param {any} eingabe
 * @param {any[]} gruppen
 * @param {any} eigeneId
 */
export function gruppenNamePruefen(eingabe, gruppen, eigeneId) {
  const name = String(eingabe ?? "").trim();
  if (!name) return { ok: false, name, fehler: "" };
  const schon = (gruppen ?? []).find(
    (g) => g.id !== eigeneId && g.name.toLowerCase() === name.toLowerCase()
  );
  if (schon) {
    return { ok: false, name, fehler: `Es gibt schon eine Projektgruppe „${schon.name}".` };
  }
  return { ok: true, name, fehler: "" };
}

/// Löst eine Gruppe auf: Alle ihre Projekte bleiben, auch archivierte, und
/// stehen danach ohne Gruppe da.
/**
 * @param {any} d
 * @param {any} id
 */
export function gruppeAufloesen(d, id) {
  for (const p of d.projekte) if (p.gruppeId === id) p.gruppeId = null;
  d.projektgruppen = d.projektgruppen.filter((/** @type {any} */ g) => g.id !== id);
}
