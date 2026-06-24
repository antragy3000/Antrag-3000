// ============================================================
// Sync-Daten ("Topf 2") – die EINZIGE Stelle, an der team-teilbare
// Daten aus dem lokalen Tresor gebaut werden.
//
// GRUNDPRINZIP (CLAUDE.md): Synchronisiert werden ausschliesslich
// UNKRITISCHE Daten. Sensible Daten (Stammdaten, IBAN, Steuer,
// Projektbeschriebe/Formular, Kostenfinanzplan/Budget, Fragebogen,
// freie Notizen) verlassen das Geraet NIE.
//
// Die Garantie ist baulich: boardAusTresor() liest NUR die unten
// ausdruecklich aufgefuehrten Felder. Es gibt keinen Code-Pfad, der
// sensible Felder ins Board uebernimmt. Der Waechter-Test
// (tools/sync-waechter.mjs) prueft das bei jeder Aenderung.
//
// AUSDRUECKLICH ERLAUBT (verlassen das Geraet):
//   - Projekt: id, name (echter Projektname, bewusste Entscheidung)
//   - je gemerkter Foerderung: Referenz (id), bei eigener Foerderung
//     Label + Geber (oeffentliche Programm-Info), Antrag-Status,
//     offizielle + eigene Fristen, Foerderer-Kontakt
//     (Ansprechpartner/E-Mail/Telefon – OHNE freie Notiz),
//     Dokument-Checkliste (Titel + Status – OHNE hochgeladenen
//     Dateinamen; die Datei selbst wird ohnehin nie gesendet)
//   - interne Fristen des Projekts (Datum + Titel)
//
// NIE im Board (sensibel, bleibt lokal/verschluesselt):
//   stammdaten, formular, kfp, fragebogen, kontakt.notiz,
//   checkliste[].datei, eigeneFoerderung.beschreibung
// ============================================================

export const BOARD_SCHEMA_VERSION = 1;

/// Foerderer-Kontakt fuer das Board: Name/E-Mail/Telefon ja, freie
/// Notiz NIE (Freitext bleibt lokal – Entscheidung des Nutzers).
function kontaktFuerBoard(k) {
  if (!k || typeof k !== "object") return null;
  const a = (k.ansprechpartner ?? "").trim();
  const e = (k.email ?? "").trim();
  const t = (k.telefon ?? "").trim();
  if (!a && !e && !t) return null;
  return { ansprechpartner: a, email: e, telefon: t };
}

/// Eine eigene (selbst recherchierte) Foerderung anhand der id finden.
function eigeneFinden(projekt, id) {
  return (projekt.eigeneFoerderungen ?? []).find((f) => f && f.id === id) ?? null;
}

/// Board-Eintrag fuer eine gemerkte Foerderung eines Projekts.
function eintragFuerFoerderung(projekt, foerderungId) {
  const a = projekt.antraege?.[foerderungId] ?? null;
  const eigen = eigeneFinden(projekt, foerderungId);

  const offizielleFristen = (a?.offizielleFristen ?? eigen?.fristen ?? [])
    .filter(Boolean);

  const eigeneFristen = (a?.eigeneFristen ?? []).map((f) =>
    typeof f === "string"
      ? { datum: f, titel: "" }
      : { datum: f?.datum ?? "", titel: (f?.titel ?? "").trim() }
  );

  // Checkliste: nur Titel + Status (kein hochgeladener Dateiname).
  const dokumente = (a?.checkliste ?? []).map((d) => ({
    text: (d?.text ?? "").trim(),
    status: d?.status ?? "noch_nicht",
    statusFrei: (d?.statusFrei ?? "").trim(),
  }));

  return {
    foerderungId,
    // Bei eigener Foerderung das oeffentliche Label/den Geber mitschicken,
    // damit das Team ohne die Datenbank weiss, worum es geht.
    eigenesLabel: eigen ? (eigen.name ?? "").trim() : null,
    eigenerGeber: eigen ? (eigen.foerdergeber ?? "").trim() : null,
    status: a?.status ?? "in_bearbeitung",
    statusFrei: (a?.statusFrei ?? "").trim(),
    offizielleFristen,
    eigeneFristen,
    kontakt: kontaktFuerBoard(a?.kontakt),
    dokumente,
  };
}

/// Board-Sicht eines Projekts (nur unkritische Felder).
function projektFuerBoard(projekt) {
  const merk = Array.isArray(projekt.merkliste) ? projekt.merkliste : [];
  return {
    id: projekt.id,
    name: (projekt.name ?? "").trim(),
    eintraege: merk.map((id) => eintragFuerFoerderung(projekt, id)),
    interneFristen: (projekt.interneFristen ?? []).map((t) => ({
      id: t?.id,
      datum: t?.datum ?? "",
      titel: (t?.titel ?? "").trim(),
    })),
  };
}

/// Baut aus den (lokalen, sensiblen) Tresor-Daten die team-teilbaren
/// Board-Daten. NUR die hier zusammengestellten Felder verlassen je
/// das Geraet. Das ist die Quelle der Wahrheit fuer den Sync.
export function boardAusTresor(daten) {
  const projekte = (daten?.projekte ?? []).map(projektFuerBoard);
  return { schema: BOARD_SCHEMA_VERSION, projekte };
}

/// Oeffentliche Projektion EINER eigenen (selbst recherchierten)
/// Foerderung fuer das Team: NUR Programm-Infos (Name, Geber, Land,
/// Webseite, Foerderhoehe, Fristen, Kriterien). Die freie BESCHREIBUNG
/// wird BEWUSST nicht uebernommen – sie bleibt lokal/verschluesselt
/// (Entscheidung des Nutzers, wie bei der Kontakt-Notiz).
function eigeneFoerderungOeffentlich(f) {
  const hk = f.harte_kriterien ?? {};
  const wk = f.weiche_kriterien ?? {};
  return {
    id: f.id,
    name: (f.name ?? "").trim(),
    foerdergeber: (f.foerdergeber ?? "").trim(),
    land: f.land ?? "ANDERES",
    webseite: (f.webseite ?? "").trim(),
    foerderhoehe_text: (f.foerderhoehe_text ?? "").trim(),
    einreichung_online: !!f.einreichung_online,
    einreich_url: (f.einreich_url ?? "").trim(),
    fristen: Array.isArray(f.fristen) ? f.fristen.filter(Boolean) : [],
    unvertraeglich_mit: Array.isArray(f.unvertraeglich_mit) ? f.unvertraeglich_mit : [],
    checkliste_vorschlag: Array.isArray(f.checkliste_vorschlag) ? f.checkliste_vorschlag : [],
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
}

/// Baut die team-teilbaren EIGENEN Foerderer (oeffentliche Felder) aus
/// dem Tresor. Wie boardAusTresor die EINZIGE Stelle, an der diese Daten
/// entstehen – der Waechter-Test prueft auch dieses Paket. Dedupe per id
/// ueber alle Projekte (dieselbe eigene Foerderung kann mehrfach gemerkt
/// sein).
export function geteilteFoerdererAusTresor(daten) {
  const gesehen = new Set();
  const foerderer = [];
  for (const p of daten?.projekte ?? []) {
    for (const f of p.eigeneFoerderungen ?? []) {
      if (!f || !f.id || gesehen.has(f.id)) continue;
      gesehen.add(f.id);
      foerderer.push({ id: f.id, inhalt: eigeneFoerderungOeffentlich(f) });
    }
  }
  return { schema: BOARD_SCHEMA_VERSION, foerderer };
}
