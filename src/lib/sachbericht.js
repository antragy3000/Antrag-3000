// ============================================================
// Sachbericht je Geldquelle: Welcher Text gilt?
//
// Das Projekt hat eine gemeinsame Vorlage (abrechnung.sachbericht), jede
// Geldquelle kann einen eigenen Text haben (quelle.sachbericht). Alles
// Tresor-Inhalt, verlässt das Gerät nie.
//
// Bewusst ohne Importe, damit tools/sachbericht-test.mjs es direkt laden kann.
// ============================================================

/// Hat diese Geldquelle einen eigenen Sachbericht? (Nur Leerzeichen zählen nicht.)
/** @param {any} q */
export function hatEigenenSachbericht(q) {
  return !!(q?.sachbericht ?? "").trim();
}

/// Der Sachbericht, der im Verwendungsnachweis dieser Quelle erscheint: ihr
/// eigener, sonst die gemeinsame Vorlage des Projekts.
/** @param {any} q */
export function sachberichtFuerQuelle(q, vorlage = "") {
  return hatEigenenSachbericht(q) ? q.sachbericht : vorlage;
}
