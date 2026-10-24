// Verbindungsprotokoll (Paket 6): ALLE Wege, auf denen die App ins Internet
// geht – mit Klartext für die Anzeige (wozu, was wird mitgeschickt).
//
// Netzverkehr gibt es nur an zwei Stellen: in den Rust-Befehlen aus
// src-tauri/src/sync.rs und im Update-Prüfer (plugin-updater). Das Fenster
// selbst darf laut CSP gar nicht ins Netz. `npm run sicherheit` prüft, dass
// jeder Rust-Netzbefehl hier steht und nur über netzAufruf()/netzMit()
// (netzProtokoll.svelte.js) aufgerufen wird – so fehlt keine Verbindung im
// Protokoll, auch wenn später neue dazukommen.
//
// Bewusst ohne Svelte-Runen, damit das Test-Skript (Node) die Liste lesen kann.

const AUSWEIS = "nur der Geräte-Ausweis (Anmeldung)";

/** @type {Record<string, { zweck: string, mit: string }>} */
export const NETZ_BEFEHLE = {
  // --- Öffentlich (Einzelplatz und alle) ---
  app_update_pruefen: { zweck: "Nach App-Update gesehen", mit: "nichts" },
  app_update_laden: { zweck: "App-Update heruntergeladen", mit: "nichts" },
  mac_update_pruefen: { zweck: "Nach App-Update gesehen (macOS)", mit: "nichts" },
  katalog_oeffentlich_holen: { zweck: "Förder-Datenbank geholt", mit: "nichts" },
  logo_oeffentlich_holen: { zweck: "Förderer-Logo geholt", mit: "nichts" },

  // --- Team (gegenseitiges TLS mit Geräte-Ausweis) ---
  sync_health: { zweck: "Team-Server erreichbar?", mit: AUSWEIS },
  sync_katalog_holen: { zweck: "Förder-Datenbank geholt (Team)", mit: AUSWEIS },
  sync_logo_holen: { zweck: "Förderer-Logo geholt (Team)", mit: AUSWEIS },
  sync_get_board: { zweck: "Team-Übersicht geholt", mit: AUSWEIS },
  sync_put_board: {
    zweck: "Projekt in die Team-Übersicht gesendet",
    mit: "Projektname, gemerkte Förderungen mit Status, Fristen, Förderer-Kontakte, Checklisten",
  },
  sync_delete_board: { zweck: "Projekt aus der Team-Übersicht entfernt", mit: "nur die Projekt-Kennung" },
  sync_meldung_senden: { zweck: "Meldung zur Förder-Datenbank gesendet", mit: "deine Meldung (Förderung, Art, Text)" },
  sync_foerderer_holen: { zweck: "Geteilte Förderer des Teams geholt", mit: AUSWEIS },
  sync_foerderer_senden: { zweck: "Eigenen Förderer mit dem Team geteilt", mit: "die öffentlichen Angaben des Förderers" },
  sync_foerderer_loeschen: { zweck: "Geteilten Förderer zurückgezogen", mit: "nur die Kennung des Förderers" },
  ausweis_erneuern: { zweck: "Geräte-Ausweis erneuert", mit: "öffentlicher Teil des neuen Geräte-Schlüssels" },
  ausweis_grace_erneuern: {
    zweck: "Abgelaufenen Geräte-Ausweis erneuert",
    mit: "alter Ausweis und öffentlicher Teil des neuen Geräte-Schlüssels",
  },
  enroll_annehmen: { zweck: "Team-Einladung eingelöst", mit: "Einladungs-Code, Gerätename, öffentlicher Geräte-Schlüssel" },
  team_erstellen: { zweck: "Team erstellt", mit: "Team-Name, Gerätename, öffentlicher Geräte-Schlüssel" },
  einladung_erstellen: { zweck: "Einladung für ein Gerät erstellt", mit: "Name des neuen Geräts" },
  mitglieder_holen: { zweck: "Team-Mitglieder geholt", mit: AUSWEIS },
  mitglied_status_setzen: { zweck: "Mitglied gesperrt oder entsperrt", mit: "Geräte-Nummer und neuer Status" },
  sync_trockenlauf: { zweck: "Sende-Vorschau an dein Mitschnitt-Programm", mit: "die Sende-Vorschau (Prüfwerkzeug)" },
};

/** Fester Update-Server (tauri.conf.json → plugins.updater.endpoints). */
export const UPDATE_SERVER = "sync.antrag3000.de";

/**
 * Server-Adresse aus den Aufruf-Argumenten eines Rust-Netzbefehls.
 * @param {Record<string, unknown>} args
 */
export function zielAusArgs(args) {
  const a = args ?? {};
  return String(a.adresse ?? a.syncAdresse ?? a.enrollUrl ?? a.teamUrl ?? a.publicUrl ?? a.zielUrl ?? "");
}

/**
 * Macht aus einer technischen Verbindungs-Fehlermeldung einen verständlichen
 * Hinweis, wo möglich. Häufigster Fall: Ein Virenschutz mit „HTTPS-/SSL-Scan"
 * (z. B. Norton) tauscht das Server-Zertifikat gegen ein eigenes aus. Die
 * öffentlichen Abrufe vertrauen dem Windows-Speicher und kommen trotzdem
 * durch; der Team-Kanal (nur Team-CA) lehnt das bewusst ab – für ihn sieht
 * es genauso aus wie ein Angriff. Dann hilft nur eine Ausnahme im Virenschutz.
 * @param {unknown} e
 */
export function netzFehlerText(e) {
  const roh = String(e ?? "");
  if (/UnknownIssuer|invalid peer certificate|certificate verify failed|unable to get local issuer/i.test(roh)) {
    return (
      "Die App hat die Verbindung abgelehnt, weil das Zertifikat des Servers nicht echt aussah. " +
      "Häufigster Grund: Dein Virenschutz (z. B. Norton, Avast, Kaspersky) prüft verschlüsselte " +
      "Verbindungen und tauscht dabei das Zertifikat aus. Lege dort eine Ausnahme für " +
      "sync.antrag3000.de und team.antrag3000.de an (meist unter „HTTPS-Scan“ oder „SSL-Scan“). " +
      "Technische Meldung: " + roh
    );
  }
  return roh;
}

/**
 * Nur der Rechnername (mit Port) einer Adresse – ohne Pfad, Zugangsdaten
 * oder Parameter. Für die Anzeige im Protokoll.
 * @param {string} adresse
 */
export function zielName(adresse) {
  const a = String(adresse ?? "").trim();
  if (!a) return "–";
  try {
    return new URL(/^[a-z]+:\/\//i.test(a) ? a : "https://" + a).host || "–";
  } catch {
    return "–";
  }
}
