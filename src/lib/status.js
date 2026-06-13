// ============================================================
// Status-Ketten (CLAUDE.md): feste Stufen plus frei beschriftbarer
// "anderer Status". Nur der aktuelle Stand, keine Historie (MVP).
//   - Antrag-Status: gilt pro Förderoption auf der Merkliste.
//   - Checklisten-Status: gilt pro benötigtem Dokument.
// ============================================================

export const ANTRAG_STATUS = [
  { key: "in_bearbeitung", label: "in Bearbeitung", farbe: "blau" },
  { key: "abgeschickt", label: "abgeschickt", farbe: "lila" },
  { key: "zusage", label: "Zusage erhalten", farbe: "gruen" },
  { key: "absage", label: "Absage erhalten", farbe: "rot" },
  { key: "nachbessern", label: "muss nachgebessert werden", farbe: "gelb" },
  { key: "anderer", label: "anderer Status …", farbe: "grau" },
];

export const CHECK_STATUS = [
  { key: "noch_nicht", label: "noch nicht bearbeitet", farbe: "grau" },
  { key: "anfragen", label: "muss angefragt werden", farbe: "gelb" },
  { key: "angefragt", label: "angefragt, warte auf Antwort", farbe: "lila" },
  { key: "alle_infos", label: "alle Infos zusammen", farbe: "blau" },
  { key: "in_bearbeitung", label: "in Bearbeitung", farbe: "blau" },
  { key: "abgeschlossen", label: "abgeschlossen", farbe: "gruen" },
  { key: "anderer", label: "anderer Status …", farbe: "grau" },
];

export const ANTRAG_STANDARD = "in_bearbeitung";
export const CHECK_STANDARD = "noch_nicht";

/// Anzeigetext eines Status (bei "anderer" der freie Text).
export function statusLabel(liste, key, frei) {
  if (key === "anderer") return (frei ?? "").trim() || "anderer Status";
  const e = liste.find((x) => x.key === key);
  return e ? e.label : (liste[0]?.label ?? "");
}

/// Farbkennung eines Status (für das farbige Etikett).
export function statusFarbe(liste, key) {
  const e = liste.find((x) => x.key === key);
  return e ? e.farbe : "grau";
}
