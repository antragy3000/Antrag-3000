// ============================================================
// Abrechnungs-Modus – Datenmodell und Helfer (Phase A1: Belege).
//
// Struktur pro Projekt (alles Tresor-Inhalt, hochsensibel – verlaesst das
// Geraet NIE):
//   abrechnung = {
//     belege:  [ { id, nr, datum, empfaenger, zweck, brutto, mwst_satz,
//                  zahlungsart, kostenstelle, status, dateien, zuordnungen,
//                  notiz } ]
//     quellen: [ ... ]   // Geldquellen (Foerderer + Eigenmittel) – Phase A4
//   }
//
// Felder, die erst in spaeteren Phasen befuellt werden, sind hier schon
// angelegt (leer), damit das Datenmodell stabil bleibt:
//   - kostenstelle (Verknuepfung mit einem KFP-Kosten-Posten)  -> Phase A3
//   - dateien      (verschluesselte Beleg-Dateien)             -> Phase A2
//   - zuordnungen  (anteilige Aufteilung auf Geldquellen)      -> Phase A4
//
// Betraege werden wie im KFP als Text gespeichert und beim Rechnen geparst
// (deutsche Schreibweise, z. B. "1.234,56"); so kann man tippen wie gewohnt.
// ============================================================

import { betragParsen, betragFormat, postenBetrag } from "./kfp.js";

export { betragFormat, betragParsen };

// Zahlungsarten (Auswahl im Beleg-Formular).
export const ZAHLUNGSARTEN = {
  bar: "Bar",
  karte: "Karte",
  ueberweisung: "Überweisung",
  lastschrift: "Lastschrift",
  sonstige: "Sonstige",
};

// Schlanke Status-Kette eines Belegs (analog zu den uebrigen Status im
// Programm). „zugeordnet"/„abgerechnet" werden in spaeteren Phasen aktiv
// genutzt; in Phase A1 ist alles erst einmal „erfasst".
export const BELEG_STATUS = {
  erfasst: "erfasst",
  zugeordnet: "zugeordnet",
  abgerechnet: "abgerechnet",
};

/// Leere Abrechnungs-Struktur fuer ein neues Projekt.
export function leereAbrechnung() {
  return { belege: [], quellen: [] };
}

/// Naechste freie laufende Beleg-Nummer (fortlaufend je Projekt).
export function naechsteBelegNr(belege) {
  let max = 0;
  for (const b of belege ?? []) {
    const n = Number(b?.nr);
    if (Number.isFinite(n) && n > max) max = n;
  }
  return max + 1;
}

/// Eindeutige Kennung fuer einen neuen Beleg.
function neueId() {
  return crypto?.randomUUID
    ? crypto.randomUUID()
    : "b-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
}

/// Frischer Beleg mit sinnvollen Vorgaben (Datum = heute).
export function neuerBeleg(nr) {
  return {
    id: neueId(),
    nr,
    datum: new Date().toISOString().slice(0, 10),
    empfaenger: "",
    zweck: "",
    brutto: "",
    mwst_satz: "",
    zahlungsart: "",
    kostenstelle: null, // Phase A3
    status: "erfasst",
    dateien: [], // Phase A2
    zuordnungen: [], // Phase A4
    notiz: "",
  };
}

/// Brutto-Betrag eines Belegs als Zahl.
export function belegBrutto(b) {
  return betragParsen(b?.brutto);
}

/// MwSt-Satz in Prozent als Zahl (0, wenn leer/ungueltig).
export function mwstSatz(b) {
  const s = betragParsen(b?.mwst_satz);
  return Number.isFinite(s) && s > 0 ? s : 0;
}

/// Netto = Brutto / (1 + Satz/100). Ohne Satz = Brutto.
export function belegNetto(b) {
  const brutto = belegBrutto(b);
  const satz = mwstSatz(b);
  return satz > 0 ? brutto / (1 + satz / 100) : brutto;
}

/// Enthaltener MwSt-Betrag = Brutto - Netto.
export function belegMwstBetrag(b) {
  return belegBrutto(b) - belegNetto(b);
}

/// Summe aller Belege (brutto).
export function belegeSumme(belege) {
  return (belege ?? []).reduce((s, b) => s + belegBrutto(b), 0);
}

// ============================================================
// Geldquellen (Phase A4): Förderer + Eigenmittel/Einnahmen. Jede Quelle hat
// ein "Soll" (bewilligter/erwarteter Betrag). Belege werden anteilig auf
// Quellen verteilt – jede Zuordnung ist { quelleId, betrag } am Beleg.
// ============================================================

export const QUELLE_TYP = {
  foerderung: "Förderer",
  eigenmittel: "Eigenmittel / Einnahmen",
};

function neueQuelleId() {
  return crypto?.randomUUID
    ? crypto.randomUUID()
    : "q-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
}

/// Frische Geldquelle.
export function neueQuelle(typ = "foerderung") {
  return { id: neueQuelleId(), typ, foerderId: "", name: "", soll: "", sachbericht: "" };
}

/// Soll-Betrag einer Quelle als Zahl.
export function quelleSoll(q) {
  return betragParsen(q?.soll);
}

/// Vorschlag für Geldquellen aus dem KFP-Finanzierungsplan: jede
/// Finanzierungs-Position wird eine Quelle (Name aus der Position, Soll =
/// ausgerechneter Betrag, Förderer wenn verknüpft). Liefert nur Quellen, die
/// noch NICHT vorhanden sind (Abgleich über foerderId bzw. Name).
export function quellenAusFinanzierung(kfp, vorhandene = []) {
  const habenFoerder = new Set(vorhandene.filter((q) => q.foerderId).map((q) => q.foerderId));
  const habenNamen = new Set(vorhandene.map((q) => (q.name || "").trim().toLowerCase()));
  const neu = [];
  for (const k of kfp?.finanzierung ?? []) {
    for (const p of k.posten ?? []) {
      const foerderId = p.foerderId || "";
      const name = (p.bezeichnung || "").trim();
      if (!name && !foerderId) continue;
      if (foerderId && habenFoerder.has(foerderId)) continue;
      if (!foerderId && name && habenNamen.has(name.toLowerCase())) continue;
      neu.push({
        id: neueQuelleId(),
        typ: foerderId ? "foerderung" : "eigenmittel",
        foerderId,
        name: name || "(Förderung)",
        soll: String(postenBetrag(p) || ""),
        sachbericht: "",
      });
      if (foerderId) habenFoerder.add(foerderId);
      if (name) habenNamen.add(name.toLowerCase());
    }
  }
  return neu;
}

/// Summe der Zuordnungen eines Belegs.
export function belegZugeordnet(b) {
  return (b?.zuordnungen ?? []).reduce((s, z) => s + betragParsen(z.betrag), 0);
}

/// Noch nicht zugeordneter ("freier") Betrag eines Belegs.
export function belegFrei(b) {
  return belegBrutto(b) - belegZugeordnet(b);
}

/// Je Quelle die Summe aller zugeordneten Beträge. Map<quelleId, Zahl>.
export function zugeordnetJeQuelle(belege) {
  const m = new Map();
  for (const b of belege ?? []) {
    for (const z of b.zuordnungen ?? []) {
      m.set(z.quelleId, (m.get(z.quelleId) ?? 0) + betragParsen(z.betrag));
    }
  }
  return m;
}

// ============================================================
// Kostenstellen = KFP-Kosten-Posten (Phase A3). Ein Beleg verweist per
// Posten-ID (Feld kostenstelle) auf einen Kosten-Posten.
// ============================================================

/// KFP-Kosten-Posten nach Kategorie gruppiert – fuer die Auswahl im
/// Beleg-Formular. Liefert [{ name, posten: [{ id, nummer, bezeichnung }] }].
export function kostenstellenNachKategorie(kfp) {
  return (kfp?.kosten ?? []).map((k, ki) => ({
    name: k.name || "(ohne Name)",
    posten: (k.posten ?? [])
      .filter((p) => p.id)
      .map((p, pi) => ({
        id: p.id,
        nummer: `${ki + 1}.${pi + 1}`,
        bezeichnung: p.bezeichnung || "(ohne Bezeichnung)",
      })),
  }));
}

/// Belegnummern je Kostenstelle: der erste Beleg der Kostenstelle "3.1"
/// erhält "3.1.1", der zweite "3.1.2" usw. Reihenfolge innerhalb der
/// Kostenstelle nach Datum (dann laufender Beleg-Nr). Wird berechnet, damit
/// die Nummer immer zum aktuellen Stand passt. Liefert Map<belegId, "3.1.1">.
/// Belege ohne (gültige) Kostenstelle bekommen keine Nummer.
export function belegNummern(belege, kfp) {
  const ksNummer = new Map(); // posten-id -> "3.1"
  let ki = 0;
  for (const k of kfp?.kosten ?? []) {
    ki += 1;
    let pi = 0;
    for (const p of k.posten ?? []) {
      pi += 1;
      if (p.id) ksNummer.set(p.id, `${ki}.${pi}`);
    }
  }

  const proKs = new Map(); // ks-id -> Belege
  for (const b of belege ?? []) {
    const ks = b.kostenstelle;
    if (ks && ksNummer.has(ks)) {
      if (!proKs.has(ks)) proKs.set(ks, []);
      proKs.get(ks).push(b);
    }
  }

  const nummern = new Map();
  for (const [ks, arr] of proKs) {
    arr.sort((a, b) => {
      const d = String(a.datum ?? "").localeCompare(String(b.datum ?? ""));
      return d !== 0 ? d : Number(a.nr) - Number(b.nr);
    });
    arr.forEach((b, i) => nummern.set(b.id, `${ksNummer.get(ks)}.${i + 1}`));
  }
  return nummern;
}

/// Baut Titel + Abschnitte für den Verwendungsnachweis EINER Geldquelle
/// (Phase A5). Wird ans Rust-Backend gegeben, das daraus PDF/Word rendert.
/// Abschnitte: Angaben, Sachbericht, Belegliste, Kostenübersicht.
/// In Tabellenzellen markiert ** am Anfang eine fette (Summen-)Zeile.
export function verwendungsnachweisAbschnitte(quelle, belege, kfp, projektName) {
  const nummern = belegNummern(belege, kfp);
  const anteil = (b) =>
    betragParsen((b.zuordnungen ?? []).find((z) => z.quelleId === quelle.id)?.betrag);

  const zugeordnet = (belege ?? [])
    .filter((b) => (b.zuordnungen ?? []).some((z) => z.quelleId === quelle.id))
    .sort((a, b) => String(a.datum ?? "").localeCompare(String(b.datum ?? "")));
  const soll = quelleSoll(quelle);
  const summe = zugeordnet.reduce((s, b) => s + anteil(b), 0);

  const titel = `Verwendungsnachweis – ${quelle.name}`;
  const abschnitte = [];

  // 1) Angaben.
  abschnitte.push({
    ueberschrift: "Angaben",
    absaetze: [],
    tabelle: [
      ["Angabe", "Wert"],
      ["Projekt", projektName || "—"],
      ["Geldquelle", quelle.name || "—"],
      ["Bewilligt (Soll)", betragFormat(soll)],
      ["Abgerechnet", betragFormat(summe)],
      ["Stand", new Date().toLocaleDateString("de-DE")],
    ],
  });

  // 2) Sachbericht (falls hinterlegt).
  if ((quelle.sachbericht ?? "").trim()) {
    abschnitte.push({ ueberschrift: "Sachbericht", absaetze: [quelle.sachbericht.trim()], tabelle: [] });
  }

  // 3) Belegliste.
  if (zugeordnet.length) {
    const zeilen = [["Beleg-Nr.", "Datum", "Beleg", "Kostenstelle", "Belegsumme", "Zugeordnet"]];
    for (const b of zugeordnet) {
      const beleg = [b.empfaenger, b.zweck].filter(Boolean).join(" · ") || "—";
      zeilen.push([
        nummern.get(b.id) ?? `#${b.nr}`,
        datumText(b.datum),
        beleg,
        kostenstelleLabel(kfp, b.kostenstelle) || "—",
        betragFormat(belegBrutto(b)),
        betragFormat(anteil(b)),
      ]);
    }
    zeilen.push(["**Summe", "", "", "", "", "**" + betragFormat(summe)]);
    abschnitte.push({ ueberschrift: "Belegliste", absaetze: [], tabelle: zeilen });
  } else {
    abschnitte.push({
      ueberschrift: "Belegliste",
      absaetze: ["Dieser Geldquelle sind noch keine Belege zugeordnet."],
      tabelle: [],
    });
  }

  // 4) Kostenübersicht: diesem Förderer zugeordnete Beträge je Kostenstelle.
  if (zugeordnet.length) {
    const proKs = new Map();
    for (const b of zugeordnet) {
      const label = kostenstelleLabel(kfp, b.kostenstelle) || "ohne Kostenstelle";
      proKs.set(label, (proKs.get(label) ?? 0) + anteil(b));
    }
    const zeilen = [["Kostenstelle", "Zugeordnet"]];
    for (const [label, betrag] of proKs) zeilen.push([label, betragFormat(betrag)]);
    zeilen.push(["**Summe", "**" + betragFormat(summe)]);
    abschnitte.push({ ueberschrift: "Kostenübersicht", absaetze: [], tabelle: zeilen });
  }

  return { titel, abschnitte };
}

/// Lesbares Etikett einer Kostenstelle (z. B. "1.2 Honorar Regie"), oder
/// "" wenn keine, bzw. "(entfernt)" wenn der Posten nicht mehr existiert.
export function kostenstelleLabel(kfp, id) {
  if (!id) return "";
  let ki = 0;
  for (const k of kfp?.kosten ?? []) {
    ki += 1;
    let pi = 0;
    for (const p of k.posten ?? []) {
      pi += 1;
      if (p.id === id) return `${ki}.${pi} ${p.bezeichnung || "(ohne Bezeichnung)"}`;
    }
  }
  return "(entfernt)";
}

/// Plan-/Ist-Uebersicht je Kostenstelle: Plan (KFP), Ist (Summe der
/// zugeordneten Belege), Rest (Plan - Ist). Plus eine Sammelzeile fuer
/// Belege ohne (gueltige) Kostenstelle und die Gesamtsummen.
export function kostenstellenUebersicht(kfp, belege) {
  const belegeArr = belege ?? [];

  // Ist je gueltiger Posten-ID aufsummieren; der Rest gilt als „unzugeordnet".
  const gueltig = new Set();
  for (const k of kfp?.kosten ?? []) for (const p of k.posten ?? []) if (p.id) gueltig.add(p.id);

  const ist = new Map(); // id -> { summe, anzahl }
  let unzugeordnetSumme = 0;
  let unzugeordnetAnzahl = 0;
  for (const b of belegeArr) {
    const betrag = belegBrutto(b);
    const ks = b.kostenstelle;
    if (ks && gueltig.has(ks)) {
      const cur = ist.get(ks) ?? { summe: 0, anzahl: 0 };
      cur.summe += betrag;
      cur.anzahl += 1;
      ist.set(ks, cur);
    } else {
      unzugeordnetSumme += betrag;
      unzugeordnetAnzahl += 1;
    }
  }

  const kategorien = (kfp?.kosten ?? []).map((k, ki) => {
    const posten = (k.posten ?? []).map((p, pi) => {
      const plan = postenBetrag(p);
      const i = ist.get(p.id) ?? { summe: 0, anzahl: 0 };
      return {
        id: p.id,
        nummer: `${ki + 1}.${pi + 1}`,
        bezeichnung: p.bezeichnung || "(ohne Bezeichnung)",
        plan,
        ist: i.summe,
        rest: plan - i.summe,
        anzahl: i.anzahl,
      };
    });
    const planSumme = posten.reduce((s, p) => s + p.plan, 0);
    const istSumme = posten.reduce((s, p) => s + p.ist, 0);
    return {
      name: k.name || "(ohne Name)",
      nummer: String(ki + 1),
      posten,
      planSumme,
      istSumme,
      restSumme: planSumme - istSumme,
    };
  });

  const planGesamt = kategorien.reduce((s, k) => s + k.planSumme, 0);
  const istGesamt = belegeArr.reduce((s, b) => s + belegBrutto(b), 0);
  return { kategorien, planGesamt, istGesamt, unzugeordnetSumme, unzugeordnetAnzahl };
}

/// Dateigröße menschlich lesbar (z. B. "1,2 MB", "340 KB").
export function groesseText(bytes) {
  const n = Number(bytes) || 0;
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1).replace(".", ",")} MB`;
}

/// Menschlich lesbares Datum ("JJJJ-MM-TT" -> "TT.MM.JJJJ").
export function datumText(iso) {
  const s = String(iso ?? "").trim();
  if (!/^\d{4}-\d{2}-\d{2}$/.test(s)) return s;
  const [j, m, t] = s.split("-");
  return `${t}.${m}.${j}`;
}
