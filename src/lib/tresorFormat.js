// Tresor-Format: reine Hilfsfunktionen rund um die Struktur des lokalen
// Datentresors – frische Vorlagen anlegen und ältere Stände auf die
// aktuelle Struktur bringen ("Migration").
//
// Bewusst OHNE eigene Oberfläche/Reaktivität: Ein-/Ausgabe-Funktionen, die
// sich leicht testen lassen. Aus +page.svelte hierher ausgelagert, damit die
// Haupt-Datei kleiner und übersichtlicher wird (Verhalten unverändert).

import { leeresFormular } from "./antrag";
import { leererKfp, neuePostenId } from "./kfp";
import { leereAbrechnung } from "./abrechnung";
import { fristNormalisieren, fristAlsDatum } from "./begriffe";
import { katalog } from "./katalog.svelte.js";

// Die editierbaren „offiziellen Fristen" im Tresor sind konkrete Datums-
// Strings (per Datumsfeld bearbeitbar). Wiederkehrende Daten ohne Jahr aus
// der Förder-Datenbank werden dabei auf das nächste konkrete Vorkommen
// aufgelöst; Hinweise bleiben am Katalog-Eintrag (Anzeige), nicht hier.
export function offizielleFristenAus(f) {
  return (f?.fristen ?? [])
    .map((e) => fristAlsDatum(fristNormalisieren(e).datum))
    .filter(Boolean)
    .map((dt) => {
      const m = String(dt.getMonth() + 1).padStart(2, "0");
      const t = String(dt.getDate()).padStart(2, "0");
      return `${dt.getFullYear()}-${m}-${t}`;
    });
}

// Eindeutige Kennung für neue Projekte.
export function neueId() {
  return crypto.randomUUID
    ? crypto.randomUUID()
    : "p-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
}

export function neuesProjekt(name) {
  return {
    id: neueId(),
    name,
    erstellt: new Date().toISOString().slice(0, 10),
    fragebogen: null,
    merkliste: [],
    formular: leeresFormular(),
    kfp: leererKfp(),
    kfpHinweisAusblenden: false,
    abrechnung: leereAbrechnung(),
    antraege: {},
    eigeneFoerderungen: [],
    interneFristen: [],
    katalogGhosts: [],
    katalogAktualisiert: [],
  };
}

// Leere Stammdaten-Struktur (alles Tresor-Inhalt, hochsensibel).
export function leereStammdaten() {
  return {
    person: { vorname: "", nachname: "", kuenstlername: "", organisation: "" },
    kontakt: { strasse: "", plz: "", ort: "", land: "", email: "", telefon: "", webseite: "" },
    bank: { kontoinhaber: "", iban: "", bic: "", bank: "" },
    steuer: { steuernummer: "", ustid: "", finanzamt: "" },
    // Logo als Data-URL (z. B. "data:image/png;base64,…"); erscheint als
    // Briefkopf in PDF und Word. Bleibt verschlüsselt im Tresor.
    logo: "",
  };
}

// Eingebackene Server-Adressen des gehosteten Modells. Zentral an EINER Stelle,
// damit die App die Anbindung von selbst kennt (der Nutzer muss nichts
// eintippen). Self-Hoster koennen den Wert im Tresor per „Erweitert"-Override
// aendern; darum sind es Standardwerte, keine harten Konstanten im Aufruf.
export const STANDARD_SERVER = "https://sync.antrag3000.de";  // oeffentlich (Katalog/Update/Enroll)
export const STANDARD_TEAM_SYNC = "team.antrag3000.de:8443";  // mTLS-Sync-Adresse (Team)

// Erkennt die ABGESCHALTETEN NAS-/Tailscale-Adressen (vor dem Netcup-Umzug),
// damit ältere Tresore automatisch auf den eingebackenen Dienst umziehen statt
// ins Leere zu laufen ("Vom Server nicht ladbar …ts.net").
function istAbgeschalteteAdresse(adr) {
  return typeof adr === "string" && /nas-yh|tail73a506|\.ts\.net/i.test(adr);
}

// Struktur eines frischen Tresors (wächst in späteren Schritten).
// Bewusst ohne Projekt: Die App fordert zum Erstellen auf.
export function frischerTresor() {
  return {
    version: 2,
    stammdaten: leereStammdaten(),
    projekte: [],
    aktivesProjektId: null,
    modus: "einzel", // "einzel" (ohne Team) oder "team"
    einzelServer: STANDARD_SERVER, // Update-/Katalog-Server (Einzelplatz), eingebacken
    sync: null,
    teamCa: null,
    katalogMeldungen: [],
    katalogStand: {},
    katalogFeldDiff: {},
  };
}

// Bringt ältere Tresor-Stände auf die aktuelle Struktur.
// Liefert true, wenn etwas geändert wurde (dann neu speichern).
export function normalisieren(d) {
  let veraendert = false;
  if (!Array.isArray(d.projekte)) {
    d.projekte = [];
    veraendert = true;
  }
  // Schritt-3-Stand: ein einzelner Fragebogen ohne Projekt.
  if (d.fragebogen) {
    const p = neuesProjekt("Mein Projekt");
    p.fragebogen = d.fragebogen;
    d.projekte.push(p);
    delete d.fragebogen;
    veraendert = true;
  }
  // Stammdaten aus aelteren Staenden um fehlende Felder ergaenzen.
  const vorlage = leereStammdaten();
  if (!d.stammdaten || typeof d.stammdaten !== "object") {
    d.stammdaten = vorlage;
    veraendert = true;
  } else {
    for (const [gruppe, felder] of Object.entries(vorlage)) {
      // Skalare Felder (z. B. logo) separat behandeln – nur ergänzen,
      // niemals einen vorhandenen Wert überschreiben.
      if (typeof felder !== "object" || felder === null) {
        if (typeof d.stammdaten[gruppe] !== "string") {
          d.stammdaten[gruppe] = felder;
          veraendert = true;
        }
        continue;
      }
      if (!d.stammdaten[gruppe] || typeof d.stammdaten[gruppe] !== "object") {
        d.stammdaten[gruppe] = felder;
        veraendert = true;
      } else {
        for (const feld of Object.keys(felder)) {
          if (typeof d.stammdaten[gruppe][feld] !== "string") {
            d.stammdaten[gruppe][feld] = "";
            veraendert = true;
          }
        }
      }
    }
  }

  // Projekte aus aelteren Staenden bekommen eine leere Merkliste
  // und ein leeres Sammel-Formular.
  for (const p of d.projekte) {
    if (!Array.isArray(p.merkliste)) {
      p.merkliste = [];
      veraendert = true;
    }
    const formularVorlage = leeresFormular();
    if (!p.formular || typeof p.formular !== "object") {
      p.formular = formularVorlage;
      veraendert = true;
    } else {
      for (const feld of Object.keys(formularVorlage)) {
        if (typeof p.formular[feld] !== "string") {
          p.formular[feld] = "";
          veraendert = true;
        }
      }
    }
    // Kostenfinanzplan ergaenzen; Texte aus den frueheren Feldern
    // "Kostenueberblick"/"Finanzierungsueberblick" hinueberretten.
    if (
      !p.kfp ||
      typeof p.kfp !== "object" ||
      !Array.isArray(p.kfp.kosten) ||
      !Array.isArray(p.kfp.finanzierung)
    ) {
      p.kfp = leererKfp();
      veraendert = true;
    }
    if (typeof p.formular.kosten === "string") {
      if (p.formular.kosten.trim()) {
        p.kfp.kosten.push({
          name: "Aus dem Formular übernommen",
          posten: [{ bezeichnung: p.formular.kosten.trim(), erlaeuterung: "", betrag: "" }],
        });
      }
      delete p.formular.kosten;
      veraendert = true;
    }
    if (typeof p.formular.finanzierung === "string") {
      if (p.formular.finanzierung.trim()) {
        p.kfp.finanzierung.push({
          name: "Aus dem Formular übernommen",
          posten: [{ bezeichnung: p.formular.finanzierung.trim(), betrag: "" }],
        });
      }
      delete p.formular.finanzierung;
      veraendert = true;
    }
    // Frueher hatten Finanzierungs-Positionen ein Status-Feld; der
    // Status gehoert zur Foerderoption (Schritt 8), nicht in den KFP.
    // Ausserdem: foerderId fuer die Verknuepfung mit der Merkliste.
    if (Array.isArray(p.kfp?.finanzierung)) {
      for (const k of p.kfp.finanzierung) {
        for (const po of k.posten ?? []) {
          if ("status" in po) {
            delete po.status;
            veraendert = true;
          }
          if (typeof po.foerderId !== "string") {
            po.foerderId = "";
            veraendert = true;
          }
        }
      }
    }
    // Kosten-Posten brauchen eine stabile ID (Kostenstelle fuer die
    // Abrechnung); aelteren Staenden nachtragen.
    if (Array.isArray(p.kfp?.kosten)) {
      for (const k of p.kfp.kosten) {
        for (const po of k.posten ?? []) {
          if (typeof po.id !== "string" || !po.id) {
            po.id = neuePostenId();
            veraendert = true;
          }
        }
      }
    }
    if (typeof p.kfpHinweisAusblenden !== "boolean") {
      p.kfpHinweisAusblenden = false;
      veraendert = true;
    }
    if (!p.antraege || typeof p.antraege !== "object" || Array.isArray(p.antraege)) {
      p.antraege = {};
      veraendert = true;
    }
    if (!Array.isArray(p.eigeneFoerderungen)) {
      p.eigeneFoerderungen = [];
      veraendert = true;
    }
    if (!Array.isArray(p.interneFristen)) {
      p.interneFristen = [];
      veraendert = true;
    }
    if (!Array.isArray(p.katalogGhosts)) {
      p.katalogGhosts = [];
      veraendert = true;
    }
    if (!Array.isArray(p.katalogAktualisiert)) {
      p.katalogAktualisiert = [];
      veraendert = true;
    }
    // Abrechnungs-Block (Belege + Geldquellen) ergaenzen, falls aus einem
    // aelteren Stand (vor dem Abrechnungs-Modus) fehlend.
    if (!p.abrechnung || typeof p.abrechnung !== "object") {
      p.abrechnung = leereAbrechnung();
      veraendert = true;
    } else {
      if (!Array.isArray(p.abrechnung.belege)) {
        p.abrechnung.belege = [];
        veraendert = true;
      }
      if (!Array.isArray(p.abrechnung.quellen)) {
        p.abrechnung.quellen = [];
        veraendert = true;
      }
      if (typeof p.abrechnung.sachbericht !== "string") {
        p.abrechnung.sachbericht = "";
        veraendert = true;
      }
    }
    // Antrag-Einträge älterer Stände um eigene Fristen ergänzen.
    const alleFoerd = [
      ...katalog.daten.foerderungen,
      ...(p.eigeneFoerderungen ?? []),
      ...(p.katalogGhosts ?? []),
    ];
    for (const [id, a] of Object.entries(p.antraege)) {
      if (!a) continue;
      if (!Array.isArray(a.eigeneFristen)) {
        a.eigeneFristen = [];
        veraendert = true;
      }
      // Eigene Fristen: alte reine Datums-Strings -> {datum, titel}.
      let umgewandelt = false;
      a.eigeneFristen = a.eigeneFristen.map((x) => {
        if (typeof x === "string") {
          umgewandelt = true;
          return { datum: x, titel: "" };
        }
        return x;
      });
      if (umgewandelt) veraendert = true;
      // Offizielle Frist(en) aus der Förderung übernehmen, falls fehlend.
      if (!Array.isArray(a.offizielleFristen)) {
        const f = alleFoerd.find((x) => x.id === id);
        a.offizielleFristen = offizielleFristenAus(f);
        veraendert = true;
      }
      if (!a.kontakt || typeof a.kontakt !== "object") {
        a.kontakt = { ansprechpartner: "", email: "", telefon: "", notiz: "" };
        veraendert = true;
      }
      // Checklisten-Punkte um das Feld "datei" (hochgeladenes Dokument)
      // ergänzen.
      if (Array.isArray(a.checkliste)) {
        for (const punkt of a.checkliste) {
          if (punkt && typeof punkt.datei !== "string") {
            punkt.datei = "";
            veraendert = true;
          }
        }
      }
    }
  }
  if (d.aktivesProjektId && !d.projekte.some((p) => p.id === d.aktivesProjektId)) {
    d.aktivesProjektId = d.projekte[0]?.id ?? null;
    veraendert = true;
  }
  if (!d.aktivesProjektId && d.projekte.length > 0) {
    d.aktivesProjektId = d.projekte[0].id;
    veraendert = true;
  }
  if (d.sync === undefined) {
    d.sync = null;
    veraendert = true;
  }
  if (d.teamCa === undefined) {
    d.teamCa = null;
    veraendert = true;
  }
  // Betriebsmodus: bestehende Tresore mit Team-Paket bleiben im Team-
  // Modus, alle anderen starten als Einzelplatz.
  if (d.modus !== "team" && d.modus !== "einzel") {
    d.modus = d.sync ? "team" : "einzel";
    veraendert = true;
  }
  if (typeof d.einzelServer !== "string") {
    d.einzelServer = STANDARD_SERVER;
    veraendert = true;
  } else if (istAbgeschalteteAdresse(d.einzelServer)) {
    // Alte NAS/Tailscale-Adresse -> auf den eingebackenen Dienst umziehen.
    d.einzelServer = STANDARD_SERVER;
    veraendert = true;
  }
  // Team-Sync-Adresse ebenso von der alten NAS auf den Dienst umziehen.
  if (d.sync && istAbgeschalteteAdresse(d.sync.adresse)) {
    d.sync.adresse = STANDARD_TEAM_SYNC;
    veraendert = true;
  }
  if (!Array.isArray(d.katalogMeldungen)) {
    d.katalogMeldungen = [];
    veraendert = true;
  }
  if (!d.katalogStand || typeof d.katalogStand !== "object" || Array.isArray(d.katalogStand)) {
    d.katalogStand = {};
    veraendert = true;
  }
  if (!d.katalogFeldDiff || typeof d.katalogFeldDiff !== "object" || Array.isArray(d.katalogFeldDiff)) {
    d.katalogFeldDiff = {};
    veraendert = true;
  }
  if (d.version !== 2) {
    d.version = 2;
    veraendert = true;
  }
  return veraendert;
}
