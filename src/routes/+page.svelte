<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open as dateiWaehlen, save as dateiSpeichern } from "@tauri-apps/plugin-dialog";
  import Foerderungen from "$lib/komponenten/Foerderungen.svelte";
  import Matching from "$lib/komponenten/Matching.svelte";
  import Merkliste from "$lib/komponenten/Merkliste.svelte";
  import Kalender from "$lib/komponenten/Kalender.svelte";
  import Stammdaten from "$lib/komponenten/Stammdaten.svelte";
  import SammelFormular from "$lib/komponenten/SammelFormular.svelte";
  import KostenPlan from "$lib/komponenten/KostenPlan.svelte";
  import Sicherung from "$lib/komponenten/Sicherung.svelte";
  import TeamSync from "$lib/komponenten/TeamSync.svelte";
  import { katalog, setzeKatalog, pruefeKatalog } from "$lib/katalog.svelte.js";
  import { leeresFormular, formularWordBauen } from "$lib/antrag";
  import { antragsPdfBauen } from "$lib/antragsPdf";
  import { leererKfp, kfpExport } from "$lib/kfp";
  import { ANTRAG_STANDARD, CHECK_STANDARD } from "$lib/status";
  import { boardAusTresor } from "$lib/sync";

  // Die App kennt fünf Ansichten:
  // laden -> einrichten (kein Tresor) ODER entsperren (Tresor da)
  // -> offen (entsperrt) | neu-aufsetzen (Passwort vergessen)
  let ansicht = $state("laden");

  let passwort = $state("");
  let passwortWdh = $state("");
  let fehler = $state("");
  let bestaetigung = $state("");
  let beschaeftigt = $state(false);

  // Die entschlüsselten Daten – leben nur im Arbeitsspeicher.
  let daten = $state(null);

  // Welcher Bereich ist nach dem Entsperren aktiv?
  let bereich = $state("alle"); // alle | passend

  // Projekt-Verwaltung
  let aktivesProjekt = $derived(
    daten ? daten.projekte.find((p) => p.id === daten.aktivesProjektId) : null
  );
  let neuesProjektOffen = $state(false);
  let neuerProjektName = $state("");
  let loeschDialogOffen = $state(false);
  let umbenennenOffen = $state(false);
  let umbenennenName = $state("");
  let sicherungOffen = $state(false);

  // Datenbank-Förderungen plus die eigenen Förderungen des aktiven
  // Projekts – diese Liste löst überall die IDs auf.
  let alleFoerderungen = $derived([
    ...katalog.daten.foerderungen,
    ...(aktivesProjekt?.eigeneFoerderungen ?? []),
  ]);

  // Eindeutige Kennung für neue Projekte.
  function neueId() {
    return crypto.randomUUID
      ? crypto.randomUUID()
      : "p-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
  }

  function neuesProjekt(name) {
    return {
      id: neueId(),
      name,
      erstellt: new Date().toISOString().slice(0, 10),
      fragebogen: null,
      merkliste: [],
      formular: leeresFormular(),
      kfp: leererKfp(),
      kfpHinweisAusblenden: false,
      antraege: {},
      eigeneFoerderungen: [],
      interneFristen: [],
    };
  }

  // Leere Stammdaten-Struktur (alles Tresor-Inhalt, hochsensibel).
  function leereStammdaten() {
    return {
      person: { vorname: "", nachname: "", kuenstlername: "", organisation: "" },
      kontakt: { strasse: "", plz: "", ort: "", land: "", email: "", telefon: "", webseite: "" },
      bank: { kontoinhaber: "", iban: "", bic: "", bank: "" },
      steuer: { steuernummer: "", ustid: "", finanzamt: "" },
    };
  }

  // Struktur eines frischen Tresors (wächst in späteren Schritten).
  // Bewusst ohne Projekt: Die App fordert zum Erstellen auf.
  function frischerTresor() {
    return {
      version: 2,
      stammdaten: leereStammdaten(),
      projekte: [],
      aktivesProjektId: null,
      sync: null,
      teamCa: null,
    };
  }

  // Bringt ältere Tresor-Stände auf die aktuelle Struktur.
  // Liefert true, wenn etwas geändert wurde (dann neu speichern).
  function normalisieren(d) {
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
      // Antrag-Einträge älterer Stände um eigene Fristen ergänzen.
      const alleFoerd = [...katalog.daten.foerderungen, ...(p.eigeneFoerderungen ?? [])];
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
          a.offizielleFristen = [...(f?.fristen ?? [])];
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
    if (d.version !== 2) {
      d.version = 2;
      veraendert = true;
    }
    return veraendert;
  }

  onMount(async () => {
    // Aktualisierten Förder-Katalog laden, falls vorhanden (Phase 3).
    // Liegt keiner vor oder passt er nicht, bleibt die mitgelieferte
    // Standard-Fassung aktiv.
    try {
      const roh = await invoke("katalog_laden");
      if (roh) {
        const obj = JSON.parse(roh);
        if (pruefeKatalog(obj).ok) setzeKatalog(obj, "datei");
      }
    } catch (e) {
      console.warn("Katalog-Override nicht ladbar:", e);
    }

    const status = await invoke("tresor_status");
    ansicht = status === "fehlt" ? "einrichten" : "entsperren";
  });

  async function einrichten(event) {
    event.preventDefault();
    fehler = "";
    if (passwort.length < 8) {
      fehler = "Das Passwort muss mindestens 8 Zeichen haben.";
      return;
    }
    if (passwort !== passwortWdh) {
      fehler = "Die beiden Passwörter stimmen nicht überein.";
      return;
    }
    beschaeftigt = true;
    try {
      const frisch = frischerTresor();
      await invoke("tresor_erstellen", {
        passwort,
        daten: JSON.stringify(frisch),
      });
      daten = frisch;
      passwort = passwortWdh = "";
      ansicht = "offen";
    } catch (e) {
      fehler = String(e);
    } finally {
      beschaeftigt = false;
    }
  }

  async function entsperren(event) {
    event.preventDefault();
    fehler = "";
    beschaeftigt = true;
    try {
      const json = await invoke("tresor_entsperren", { passwort });
      daten = JSON.parse(json);
      // Ältere Datenstände (z. B. aus Schritt 3) sanft überführen.
      if (normalisieren(daten)) await tresorSpeichern();
      passwort = "";
      ansicht = "offen";
    } catch (e) {
      fehler =
        String(e) === "falsches_passwort" ? "Falsches Passwort." : String(e);
      passwort = "";
    } finally {
      beschaeftigt = false;
    }
  }

  async function sperren() {
    syncLoopStoppen();
    syncVerbunden = false;
    syncMeldung = null;
    zuletztGeprueft = null;
    await invoke("tresor_sperren");
    daten = null;
    fehler = "";
    bereich = "alle";
    ansicht = "entsperren";
  }

  // Nach dem Einspielen einer Sicherung: abmelden, der wiederhergestellte
  // Tresor wird mit seinem Passwort neu entsperrt.
  async function nachWiederherstellung() {
    daten = null;
    fehler = "";
    bereich = "alle";
    sicherungOffen = false;
    ansicht = "entsperren";
  }

  // Speichert den gesamten Datenstand verschlüsselt in den Tresor.
  async function tresorSpeichern() {
    await invoke("tresor_speichern", {
      daten: JSON.stringify($state.snapshot(daten)),
    });
  }

  // Fragebogen-Antworten gehören zum aktiven Projekt und damit in
  // den Tresor (Budget ist sensibel).
  async function fragebogenSpeichern(antworten) {
    aktivesProjekt.fragebogen = antworten;
    await tresorSpeichern();
  }

  // Entfernt das aktive Projekt endgültig. Ohne verbleibende Projekte
  // zeigt die App die Aufforderung, ein neues zu erstellen.
  async function projektLoeschen() {
    daten.projekte = daten.projekte.filter((p) => p.id !== daten.aktivesProjektId);
    daten.aktivesProjektId = daten.projekte[0]?.id ?? null;
    loeschDialogOffen = false;
    await tresorSpeichern();
  }

  // Stammdaten ersetzen und verschlüsselt sichern.
  async function stammdatenSpeichern(neu) {
    daten.stammdaten = neu;
    await tresorSpeichern();
  }

  // Sammel-Formular des aktiven Projekts ersetzen und sichern.
  async function formularSpeichern(neu) {
    aktivesProjekt.formular = neu;
    await tresorSpeichern();
  }

  // Kostenfinanzplan des aktiven Projekts ersetzen und verschlüsselt
  // sichern. Die Excel wird hier bewusst NICHT geschrieben.
  async function kfpSpeichern(neu) {
    aktivesProjekt.kfp = neu;
    await tresorSpeichern();
  }

  // Excel des Kostenfinanzplans NUR auf ausdrücklichen Wunsch erzeugen.
  // Sie liegt danach unverschlüsselt im Projektordner. Gibt den Pfad
  // zurück. (Datensouveränität: bewusste, informierte Entscheidung.)
  async function kfpExcelErzeugen(kfpDaten) {
    return await invoke("kfp_excel_schreiben", {
      projekt: aktivesProjekt.name,
      kfp: kfpExport(kfpDaten),
    });
  }

  // Merkt pro Projekt, dass der Sensibel-Hinweis nicht mehr nötig ist.
  async function kfpHinweisMerken() {
    aktivesProjekt.kfpHinweisAusblenden = true;
    await tresorSpeichern();
  }

  // Word aus dem Sammel-Formular im PROJEKT-Ordner erzeugen – ohne
  // Stammdaten, ohne KFP. Bekommt die aktuell im Formular angezeigten
  // Daten übergeben (auch ungespeicherte Änderungen auf dem Bildschirm).
  async function formularWordErzeugen(formularDaten) {
    const { titel, warnhinweis, abschnitte } = formularWordBauen(
      formularDaten,
      aktivesProjekt.name
    );
    await invoke("formular_word_erzeugen", {
      projekt: aktivesProjekt.name,
      titel,
      warnhinweis,
      abschnitte,
    });
  }

  // Dokument zu einem Checklisten-Punkt hochladen: Datei wählen, von
  // Rust in [Projekt]/[Förderer]/Dateien/ kopieren und einheitlich
  // umbenennen. Gibt den neuen Dateinamen zurück (oder null).
  async function dokumentHochladen(foerderungName, dokumentart) {
    const pfad = await dateiWaehlen({
      title: "Dokument auswählen (PDF oder Bild)",
      multiple: false,
      filters: [{ name: "PDF oder Bild", extensions: ["pdf", "jpg", "jpeg", "png"] }],
    });
    if (!pfad) return null; // abgebrochen
    try {
      return await invoke("dokument_hochladen", {
        projekt: aktivesProjekt.name,
        foerderung: foerderungName,
        dokumentart,
        quelle: pfad,
      });
    } catch (e) {
      alert("Das Dokument konnte nicht hochgeladen werden.\n" + e);
      return null;
    }
  }

  // Baut die Aufruf-Argumente fürs Antrags-PDF (Stammblatt, Formular,
  // KFP, Anhang-Liste + hochgeladene Dateien dieser Förderung).
  function pdfArgs(foerderung) {
    const antrag = aktivesProjekt.antraege[foerderung.id];
    const { titel, abschnitte, anhaenge } = antragsPdfBauen(
      $state.snapshot(daten.stammdaten),
      $state.snapshot(aktivesProjekt.formular),
      $state.snapshot(aktivesProjekt.kfp),
      $state.snapshot(foerderung),
      $state.snapshot(antrag?.checkliste ?? [])
    );
    return { projekt: aktivesProjekt.name, foerderung: foerderung.name, titel, abschnitte, anhaenge };
  }

  // Vorschau erzeugen und im PDF-Programm öffnen.
  async function antragsPdfVorschau(foerderung) {
    try {
      await invoke("antrags_pdf_vorschau", pdfArgs(foerderung));
      return true;
    } catch (e) {
      alert("Die Vorschau konnte nicht erstellt werden.\n" + e);
      return false;
    }
  }

  // Endgültiges PDF in den Förderer-Ordner speichern. Gibt den Pfad zurück.
  async function antragsPdfSpeichern(foerderung) {
    try {
      return await invoke("antrags_pdf_speichern", pdfArgs(foerderung));
    } catch (e) {
      alert("Das Antrags-PDF konnte nicht gespeichert werden.\n" + e);
      return null;
    }
  }

  // --- Team-Synchronisation (Phase 2) ---
  // Zugangs-Paket laden: Datei wählen, in Rust prüfen (Ausweis +
  // Gerätename aus dem Zertifikat), verschlüsselt im Tresor ablegen.
  async function zugangspaketLaden() {
    const pfad = await dateiWaehlen({
      title: "Zugangs-Paket wählen",
      multiple: false,
      filters: [{ name: "Zugangs-Paket", extensions: ["a3kpaket"] }],
    });
    if (!pfad) return null;
    try {
      const info = await invoke("zugangspaket_pruefen", { pfad });
      daten.sync = {
        adresse: info.adresse,
        geraetName: info.geraet_name,
        ausweisPem: info.ausweis_pem,
        letzterAbgleich: null,
      };
      await tresorSpeichern();
      return info;
    } catch (e) {
      alert("Das Zugangs-Paket konnte nicht geladen werden.\n" + e);
      return null;
    }
  }

  // Verbindungstest gegen den Team-Server (mTLS GET /api/health).
  async function syncVerbindungTesten() {
    if (!daten.sync) return { ok: false, fehler: "Kein Zugangs-Paket geladen." };
    try {
      const ok = await invoke("sync_health", {
        adresse: daten.sync.adresse,
        ausweisPem: daten.sync.ausweisPem,
      });
      return { ok, fehler: ok ? null : "Server erreichbar, aber unerwartete Antwort." };
    } catch (e) {
      return { ok: false, fehler: String(e) };
    }
  }

  async function zugangspaketEntfernen() {
    syncLoopStoppen();
    syncVerbunden = false;
    syncMeldung = null;
    zuletztGeprueft = null;
    daten.sync = null;
    await tresorSpeichern();
  }

  // --- Team verwalten (Admin): CA + Zugangs-Pakete in der App erzeugen ---
  async function teamCaErstellen(adresse) {
    try {
      const ca = await invoke("team_ca_erstellen");
      daten.teamCa = { certPem: ca.cert_pem, keyPem: ca.key_pem, adresse: adresse.trim() };
      await tresorSpeichern();
      return true;
    } catch (e) {
      alert("Die Team-CA konnte nicht erstellt werden.\n" + e);
      return false;
    }
  }

  // Öffentliches CA-Zertifikat als Datei speichern (für die NAS / Caddy).
  async function teamCaExportieren() {
    if (!daten.teamCa) return;
    const ziel = await dateiSpeichern({
      title: "Team-CA-Zertifikat speichern",
      defaultPath: "team-ca.crt",
      filters: [{ name: "Zertifikat", extensions: ["crt"] }],
    });
    if (!ziel) return;
    try {
      await invoke("team_ca_cert_exportieren", { certPem: daten.teamCa.certPem, ziel });
      alert("Gespeichert:\n" + ziel + "\n\nDiese Datei kommt zu Caddy auf die NAS.");
    } catch (e) {
      alert("Konnte nicht gespeichert werden.\n" + e);
    }
  }

  // Zugangs-Paket für ein (anderes) Gerät erzeugen und als Datei speichern.
  async function geraetPaketErstellen(geraetName) {
    if (!daten.teamCa) return;
    const sicher = (geraetName.trim().replace(/[^A-Za-z0-9_.-]/g, "_")) || "Geraet";
    const ziel = await dateiSpeichern({
      title: "Zugangs-Paket speichern",
      defaultPath: sicher + ".a3kpaket",
      filters: [{ name: "Zugangs-Paket", extensions: ["a3kpaket"] }],
    });
    if (!ziel) return;
    try {
      await invoke("geraet_paket_speichern", {
        caCertPem: daten.teamCa.certPem,
        caKeyPem: daten.teamCa.keyPem,
        geraetName: geraetName.trim(),
        adresse: daten.teamCa.adresse,
        ziel,
      });
      alert("Zugangs-Paket gespeichert:\n" + ziel + "\n\nGib es offline an das Gerät weiter (z. B. USB) – nicht per Mail.");
    } catch (e) {
      alert("Das Zugangs-Paket konnte nicht erstellt werden.\n" + e);
    }
  }

  // Dieses Gerät direkt einrichten (ohne Datei-Umweg).
  async function diesesGeraetEinrichten(geraetName) {
    if (!daten.teamCa) return null;
    try {
      const info = await invoke("geraet_paket_direkt", {
        caCertPem: daten.teamCa.certPem,
        caKeyPem: daten.teamCa.keyPem,
        geraetName: geraetName.trim(),
        adresse: daten.teamCa.adresse,
      });
      daten.sync = {
        adresse: info.adresse,
        geraetName: info.geraet_name,
        ausweisPem: info.ausweis_pem,
        letzterAbgleich: null,
      };
      await tresorSpeichern();
      return info;
    } catch (e) {
      alert("Das Gerät konnte nicht eingerichtet werden.\n" + e);
      return null;
    }
  }

  // --- Etappe 4b/4c: Fortlaufender Abgleich (Start/Stopp) ---
  // Nach dem Start synchronisiert die App von selbst weiter: in einem
  // kurzen Takt werden GEÄNDERTE eigene Projekte hochgeladen und das
  // Team-Board geholt. Echte "Echtzeit" über eine Dauerverbindung
  // (WebSocket/SSE) wäre für ein Board, das sich nur wenige Male am Tag
  // ändert, unverhältnismäßig: persistente mTLS-Verbindung, Server-Push,
  // Reconnect-Logik. Ein 10-Sekunden-Takt fühlt sich live an und hält die
  // NAS-Last winzig (im Ruhezustand 1 kleine Abfrage alle 10 s, sonst
  // nichts). Der Wert ist eine Konstante und leicht anzupassen.
  const SYNC_INTERVALL_MS = 10000;

  let syncLaeuft = $state(false);    // läuft die Dauer-Synchronisation?
  let syncVerbunden = $state(false); // stand die Verbindung beim letzten Versuch?
  let syncMeldung = $state(null);    // { art: "ok"|"warn"|"info", text }
  let zuletztGeprueft = $state(null); // Zeitpunkt des letzten Takts (Liveness)
  let syncTimer = null;              // Handle des nächsten Takts (kein $state)
  let tickAktiv = false;            // verhindert überlappende Takte

  // Einmaliger Verbindungstest; aktualisiert syncVerbunden und gibt das
  // {ok, fehler}-Ergebnis zurück (von "Verbindung testen" genutzt).
  async function verbindungPruefen() {
    if (!daten?.sync) {
      syncVerbunden = false;
      return { ok: false, fehler: "Kein Zugangs-Paket geladen." };
    }
    const r = await syncVerbindungTesten();
    syncVerbunden = !!r.ok;
    return r;
  }

  // Sync-Protokoll (nur im Speicher, max. 50 Einträge): zeigt transparent,
  // was tatsächlich gesendet/gelöscht wurde. Wird nicht in den Tresor
  // geschrieben (reine Anzeige).
  let protokoll = $state([]);
  function protokollEintrag(e) {
    protokoll.unshift({ zeit: new Date().toISOString(), ...e });
    if (protokoll.length > 50) protokoll.length = 50;
  }

  // Ein Durchlauf: gelöschte Projekte abmelden, nur tatsächlich geänderte
  // eigene Projekte senden, danach das Team-Board holen. Schreibt den
  // Tresor nur bei Änderung, damit der Takt im Ruhezustand keine Datei-/
  // Krypto-Last erzeugt.
  async function einAbgleich() {
    const adresse = daten.sync.adresse;
    const ausweisPem = daten.sync.ausweisPem;
    const versionen = { ...(daten.sync.versionen ?? {}) };
    const gesendet = { ...(daten.sync.gesendet ?? {}) };
    let geaendert = false;
    let konflikte = 0;
    const protZeilen = [];

    const board = boardAusTresor($state.snapshot(daten));
    const aktuelleIds = new Set(board.projekte.map((p) => p.id));

    // 1. Löschungen: früher gesendete Projekte, die es lokal nicht mehr
    //    gibt, vom Team-Board entfernen (sonst bleiben sie als Leiche).
    for (const id of Object.keys(gesendet)) {
      if (aktuelleIds.has(id)) continue;
      await invoke("sync_delete_board", { adresse, ausweisPem, projektId: id });
      delete gesendet[id];
      delete versionen[id];
      protZeilen.push({ projektId: id, aktion: "gelöscht", bytes: 0, body: null });
      geaendert = true;
    }

    // 2. Hochladen – nur, wenn sich die Board-Sicht des Projekts seit dem
    //    letzten Senden geändert hat (Vergleich über die exakte JSON).
    for (const p of board.projekte) {
      const js = JSON.stringify(p);
      if (gesendet[p.id] === js) continue; // unverändert → nicht senden
      const body = JSON.stringify({ inhalt: p, basis_version: versionen[p.id] ?? null });
      const antwortText = await invoke("sync_put_board", {
        adresse, ausweisPem, projektId: p.id, bodyJson: body,
      });
      const antwort = JSON.parse(antwortText);
      versionen[p.id] = antwort.version;
      if (antwort.konflikt) konflikte++;
      else gesendet[p.id] = js;
      protZeilen.push({
        projektId: p.id,
        aktion: antwort.konflikt ? "Konflikt – übersprungen" : "gesendet",
        bytes: body.length,
        body,
      });
      geaendert = true;
    }

    // 3. Team-Board holen.
    const boardText = await invoke("sync_get_board", { adresse, ausweisPem });
    const serverBoard = JSON.parse(boardText);
    for (const row of serverBoard) versionen[row.projekt_id] = row.version;
    if (JSON.stringify(daten.sync.teamBoard ?? null) !== JSON.stringify(serverBoard)) {
      daten.sync.teamBoard = serverBoard;
      geaendert = true;
    }

    // Nur protokollieren, wenn etwas gesendet/gelöscht wurde (kein Spam
    // bei reinen Abfrage-Takten).
    if (protZeilen.length > 0) {
      protokollEintrag({ zeilen: protZeilen, geholt: serverBoard.length });
    }

    if (geaendert) {
      daten.sync.versionen = versionen;
      daten.sync.gesendet = gesendet;
      daten.sync.letzterAbgleich = new Date().toISOString();
      await tresorSpeichern();
    }
    return { konflikte };
  }

  // Ein Takt der Dauer-Synchronisation; plant sich selbst neu, solange
  // syncLaeuft. Bricht bei Netzfehlern NICHT ab, sondern versucht weiter
  // (so erholt sich der Sync von selbst, wenn die NAS zurückkommt).
  async function syncTakt() {
    if (!syncLaeuft || tickAktiv) return;
    tickAktiv = true;
    try {
      const r = await einAbgleich();
      syncVerbunden = true;
      zuletztGeprueft = new Date().toISOString();
      syncMeldung = {
        art: r.konflikte > 0 ? "warn" : "ok",
        text: r.konflikte > 0
          ? `Aktiv · ${r.konflikte} Konflikt(e) übersprungen`
          : "Aktiv · synchronisiert",
      };
    } catch (e) {
      syncVerbunden = false;
      syncMeldung = { art: "warn", text: "Verbindung unterbrochen – versuche weiter …" };
    } finally {
      tickAktiv = false;
      if (syncLaeuft) syncTimer = setTimeout(syncTakt, SYNC_INTERVALL_MS);
    }
  }

  // Loop stoppen ohne Tresor-Schreiben (für Sperren/Abmelden).
  function syncLoopStoppen() {
    syncLaeuft = false;
    if (syncTimer) { clearTimeout(syncTimer); syncTimer = null; }
  }

  async function autoSyncStarten() {
    if (!daten?.sync || syncLaeuft) return;
    const r = await verbindungPruefen();
    if (!r.ok) {
      syncMeldung = { art: "warn", text: "Verbindung steht noch nicht – zuerst „Verbindung testen“." };
      return;
    }
    syncLaeuft = true;
    syncMeldung = { art: "info", text: "Synchronisation gestartet." };
    syncTakt(); // sofort der erste Durchlauf, dann im Takt weiter
  }

  function autoSyncStoppen() {
    syncLoopStoppen();
    syncMeldung = { art: "info", text: "Synchronisation gestoppt." };
  }

  // --- Etappe 5: Trockenlauf / Transparenz ---
  // Baut die EXAKTEN Sende-Körper, die der echte Sync hochladen würde –
  // ohne irgendetwas zu senden. Quelle ist allein boardAusTresor().
  function trockenlaufKoerper() {
    const board = boardAusTresor($state.snapshot(daten ?? {}));
    return board.projekte.map((p) =>
      JSON.stringify({ inhalt: p, basis_version: null }, null, 2),
    );
  }

  // Schickt dieselben Körper an einen lokalen Mitschnitt-Server (ohne
  // Ausweis/TLS), damit man unabhängig sieht, was ins Netz ginge.
  async function trockenlaufSenden(zielUrl) {
    const koerper = trockenlaufKoerper().map((s) => JSON.stringify(JSON.parse(s)));
    try {
      const n = await invoke("sync_trockenlauf", { zielUrl, koerper });
      protokollEintrag({
        trockenlauf: true,
        ziel: zielUrl,
        zeilen: koerper.map((b, i) => ({
          projektId: "(Projekt " + (i + 1) + ")",
          aktion: "Trockenlauf gesendet",
          bytes: b.length,
          body: b,
        })),
        geholt: 0,
      });
      return { ok: true, n };
    } catch (e) {
      return { ok: false, fehler: String(e) };
    }
  }

  // ids meiner lokalen Projekte – damit die Team-Übersicht markieren kann,
  // welche Einträge von diesem Gerät stammen.
  let meineProjektIds = $derived(
    daten ? (daten.projekte ?? []).map((p) => p.id) : [],
  );

  // Klarname einer Förderung für die Team-Übersicht: bei eigener Förderung
  // das mitgeschickte Label, sonst aus der (öffentlichen) Förder-Datenbank.
  function boardFoerderungLabel(eintrag) {
    if (eintrag?.eigenesLabel) return eintrag.eigenesLabel;
    const f = katalog.daten.foerderungen.find((x) => x.id === eintrag?.foerderungId);
    return f ? f.name : "Förderung";
  }

  // Liefert (und erstellt bei Bedarf) den Antrag-Status-Eintrag einer
  // gemerkten Förderung. Die Checkliste startet mit den üblichen
  // Unterlagen der Förderung.
  function antragHolen(foerderung) {
    let a = aktivesProjekt.antraege[foerderung.id];
    if (!a) {
      a = {
        status: ANTRAG_STANDARD,
        statusFrei: "",
        offizielleFristen: [...(foerderung.fristen ?? [])],
        eigeneFristen: [],
        kontakt: { ansprechpartner: "", email: "", telefon: "", notiz: "" },
        checkliste: (foerderung.checkliste_vorschlag ?? []).map((t) => ({
          text: t,
          status: CHECK_STANDARD,
          statusFrei: "",
          datei: "",
        })),
      };
      aktivesProjekt.antraege[foerderung.id] = a;
    }
    // Offizielle Frist(en): aus der Datenbank vorbefüllt, aber editierbar.
    if (!Array.isArray(a.offizielleFristen)) {
      a.offizielleFristen = [...(foerderung.fristen ?? [])];
    }
    if (!Array.isArray(a.eigeneFristen)) a.eigeneFristen = [];
    // Eigene Fristen: alte reine Datums-Form -> {datum, titel}.
    a.eigeneFristen = a.eigeneFristen.map((x) =>
      typeof x === "string" ? { datum: x, titel: "" } : x
    );
    if (!a.kontakt || typeof a.kontakt !== "object") {
      a.kontakt = { ansprechpartner: "", email: "", telefon: "", notiz: "" };
    }
    return a;
  }

  // Eigene (selbst recherchierte) Förderung anlegen und direkt auf die
  // Merkliste setzen. Liegt verschlüsselt im Projekt (Tresor).
  async function eigeneFoerderungAnlegen(eingabe) {
    const f = {
      id: "eigen-" + neueId(),
      eigen: true,
      name: eingabe.name.trim(),
      foerdergeber: eingabe.foerdergeber.trim(),
      land: eingabe.land || "ANDERES",
      beschreibung: eingabe.beschreibung.trim(),
      webseite: eingabe.webseite.trim(),
      foerderhoehe_text: eingabe.foerderhoehe.trim() || "—",
      fristen: eingabe.frist ? [eingabe.frist] : [],
      unvertraeglich_mit: [],
      checkliste_vorschlag: (eingabe.dokumente ?? [])
        .map((s) => s.trim())
        .filter(Boolean),
      harte_kriterien: {
        wohnsitz: [],
        durchfuehrungsort: [],
        traegerschaft: ["einzelperson", "gruppe", "organisation"],
        studentisch_erlaubt: true,
      },
      weiche_kriterien: {
        sparten: [],
        projektarten: [],
        budget_min: null,
        budget_max: null,
        waehrung: "EUR",
        zeitpunkt: eingabe.laufend ? "laufend" : "fristen",
      },
    };
    aktivesProjekt.eigeneFoerderungen.push(f);
    if (!aktivesProjekt.merkliste.includes(f.id)) {
      aktivesProjekt.merkliste.push(f.id);
    }
    await tresorSpeichern();
  }

  // Interne Frist (unabhängig von Förderungen) anlegen / entfernen.
  async function interneFristAnlegen(eingabe) {
    aktivesProjekt.interneFristen.push({
      id: neueId(),
      datum: eingabe.datum,
      titel: eingabe.titel.trim() || "Interner Termin",
    });
    await tresorSpeichern();
  }
  async function interneFristEntfernen(id) {
    aktivesProjekt.interneFristen = aktivesProjekt.interneFristen.filter((t) => t.id !== id);
    await tresorSpeichern();
  }

  // Förderung auf die Merkliste des aktiven Projekts setzen bzw.
  // wieder entfernen (Stern-Knopf).
  async function merklisteUmschalten(id) {
    if (!aktivesProjekt) {
      neuesProjektOffen = true;
      return;
    }
    const liste = aktivesProjekt.merkliste;
    const entfernen = liste.includes(id);
    aktivesProjekt.merkliste = entfernen
      ? liste.filter((x) => x !== id)
      : [...liste, id];
    // Eigene Förderungen existieren nur über die Merkliste: beim
    // Entfernen werden sie ganz gelöscht (sie stehen nicht in der DB).
    if (entfernen && id.startsWith("eigen-")) {
      aktivesProjekt.eigeneFoerderungen = aktivesProjekt.eigeneFoerderungen.filter(
        (f) => f.id !== id
      );
      delete aktivesProjekt.antraege[id];
    }
    await tresorSpeichern();
  }

  function umbenennenOeffnen() {
    umbenennenName = aktivesProjekt.name;
    umbenennenOffen = true;
  }

  async function projektUmbenennen(event) {
    event.preventDefault();
    const name = umbenennenName.trim();
    if (!name) return;
    const alterName = aktivesProjekt.name;
    aktivesProjekt.name = name;
    umbenennenOffen = false;
    await tresorSpeichern();
    // Falls es schon einen Projektordner gibt, zieht er mit um.
    try {
      await invoke("ordner_umbenennen", { alt: alterName, neu: name });
    } catch (e) {
      alert("Hinweis: Der Projektordner konnte nicht umbenannt werden.\n" + e);
    }
  }

  // Legt den Ordner des aktiven Projekts (optional mit Foerderungs-
  // Unterordner) bei Bedarf an und oeffnet ihn im Explorer.
  async function ordnerOeffnen(foerderungsName = null) {
    try {
      await invoke("ordner_oeffnen", {
        projekt: aktivesProjekt.name,
        foerderung: foerderungsName,
      });
    } catch (e) {
      alert("Der Ordner konnte nicht geöffnet werden.\n" + e);
    }
  }

  async function projektAnlegen(event) {
    event.preventDefault();
    const name = neuerProjektName.trim();
    if (!name) return;
    const p = neuesProjekt(name);
    daten.projekte.push(p);
    daten.aktivesProjektId = p.id;
    neuerProjektName = "";
    neuesProjektOffen = false;
    await tresorSpeichern();
  }

  async function neuAufsetzen() {
    fehler = "";
    beschaeftigt = true;
    try {
      const datum = new Date().toISOString().slice(0, 10); // z. B. 2026-06-12
      await invoke("tresor_neu_aufsetzen", { datum });
      bestaetigung = "";
      ansicht = "einrichten";
    } catch (e) {
      fehler = String(e);
    } finally {
      beschaeftigt = false;
    }
  }
</script>

{#if ansicht === "laden"}
  <div class="buehne"></div>
{:else if ansicht === "einrichten"}
  <div class="buehne">
    <div class="karte">
      <h1>Antrag 3000</h1>
      <p class="untertitel">Willkommen! Lege zuerst dein Tresor-Passwort fest.</p>

      <form onsubmit={einrichten}>
        <label for="pw1">Passwort (mindestens 8 Zeichen)</label>
        <input id="pw1" type="password" bind:value={passwort} autocomplete="new-password" />

        <label for="pw2">Passwort wiederholen</label>
        <input id="pw2" type="password" bind:value={passwortWdh} autocomplete="new-password" />

        {#if fehler}<p class="fehler">{fehler}</p>{/if}

        <button type="submit" disabled={beschaeftigt}>
          {beschaeftigt ? "Tresor wird angelegt …" : "Tresor anlegen"}
        </button>
      </form>

      <p class="warnung">
        Wichtig: Dieses Passwort kann <strong>nicht wiederhergestellt</strong> werden.
        Ohne Passwort sind die Daten endgültig verloren – so ist es gewollt,
        damit niemand außer dir an deine Daten kommt.
      </p>
    </div>
  </div>
{:else if ansicht === "entsperren"}
  <div class="buehne">
    <div class="karte">
      <h1>Antrag 3000</h1>
      <p class="untertitel">Tresor entsperren</p>

      <form onsubmit={entsperren}>
        <label for="pw">Passwort</label>
        <input id="pw" type="password" bind:value={passwort} autocomplete="current-password" />

        {#if fehler}<p class="fehler">{fehler}</p>{/if}

        <button type="submit" disabled={beschaeftigt || passwort.length === 0}>
          {beschaeftigt ? "Wird geprüft …" : "Entsperren"}
        </button>
      </form>

      <button class="leise" onclick={() => { fehler = ""; bestaetigung = ""; ansicht = "neu-aufsetzen"; }}>
        Passwort vergessen? Neu aufsetzen …
      </button>
    </div>
  </div>
{:else if ansicht === "neu-aufsetzen"}
  <div class="buehne">
    <div class="karte">
      <h1>Neu aufsetzen</h1>
      <p class="untertitel warntext">
        Dein bisheriger Tresor wird beiseitegelegt (nicht gelöscht) und du
        beginnst mit einem leeren Tresor und neuem Passwort von vorn.
        Ohne das alte Passwort bleiben die alten Daten unlesbar.
      </p>

      <label for="bestaetigung">Tippe zur Bestätigung: <strong>NEU AUFSETZEN</strong></label>
      <input id="bestaetigung" type="text" bind:value={bestaetigung} />

      {#if fehler}<p class="fehler">{fehler}</p>{/if}

      <button
        class="gefahr"
        disabled={bestaetigung !== "NEU AUFSETZEN" || beschaeftigt}
        onclick={neuAufsetzen}
      >
        Ja, neu aufsetzen
      </button>
      <button class="leise" onclick={() => { fehler = ""; ansicht = "entsperren"; }}>
        Abbrechen
      </button>
    </div>
  </div>
{:else if ansicht === "offen"}
  <div class="app">
    <header>
      <div class="links">
        <span class="logo">Antrag 3000</span>
        <div class="projektwahl">
          {#if daten.projekte.length === 0}
            <button class="leise" onclick={() => (neuesProjektOffen = true)}>
              + Erstes Projekt erstellen
            </button>
          {:else}
            <select bind:value={daten.aktivesProjektId} onchange={tresorSpeichern}>
              {#each daten.projekte as p (p.id)}
                <option value={p.id}>{p.name}</option>
              {/each}
            </select>
            <button class="leise" onclick={() => (neuesProjektOffen = true)}>
              + Projekt
            </button>
            <button
              class="leise"
              title="Aktives Projekt umbenennen"
              aria-label="Aktives Projekt umbenennen"
              onclick={umbenennenOeffnen}
            >
              ✏️
            </button>
            <button
              class="leise"
              title="Aktives Projekt löschen"
              aria-label="Aktives Projekt löschen"
              onclick={() => (loeschDialogOffen = true)}
            >
              🗑
            </button>
          {/if}
        </div>
      </div>
      <nav>
        <button class:aktiv={bereich === "alle"} onclick={() => (bereich = "alle")}>
          Alle Förderungen
        </button>
        <button class:aktiv={bereich === "passend"} onclick={() => (bereich = "passend")}>
          Passende für mich
        </button>
        <span class="nav-trenner" aria-hidden="true"></span>
        <button class:aktiv={bereich === "merkliste"} onclick={() => (bereich = "merkliste")}>
          Merkliste{#if aktivesProjekt?.merkliste.length}&nbsp;({aktivesProjekt.merkliste.length}){/if}
        </button>
        <button class:aktiv={bereich === "fristen"} onclick={() => (bereich = "fristen")}>
          Fristen
        </button>
        <button class:aktiv={bereich === "formular"} onclick={() => (bereich = "formular")}>
          Formular
        </button>
        <button class:aktiv={bereich === "kostenplan"} onclick={() => (bereich = "kostenplan")}>
          Kostenplan
        </button>
        <span class="nav-trenner" aria-hidden="true"></span>
        <button class:aktiv={bereich === "stammdaten"} onclick={() => (bereich = "stammdaten")}>
          Stammdaten
        </button>
        <button class:aktiv={bereich === "teamsync"} onclick={() => (bereich = "teamsync")}>
          Team-Sync
        </button>
      </nav>
      <div class="rechts">
        <button class="leise" onclick={() => (sicherungOffen = true)}>🛡 Sicherung</button>
        <button class="leise" onclick={sperren}>Sperren</button>
      </div>
    </header>
    <main>
      {#if bereich === "alle"}
        <Foerderungen
          merkliste={aktivesProjekt?.merkliste ?? null}
          umschalten={merklisteUmschalten}
        />
      {:else if bereich === "stammdaten"}
        <Stammdaten stammdaten={daten.stammdaten} speichern={stammdatenSpeichern} />
      {:else if bereich === "teamsync"}
        <TeamSync
          sync={daten.sync}
          teamCa={daten.teamCa}
          laden={zugangspaketLaden}
          testen={verbindungPruefen}
          entfernen={zugangspaketEntfernen}
          caErstellen={teamCaErstellen}
          caExportieren={teamCaExportieren}
          paketErstellen={geraetPaketErstellen}
          geraetEinrichten={diesesGeraetEinrichten}
          starten={autoSyncStarten}
          stoppen={autoSyncStoppen}
          pruefen={verbindungPruefen}
          {syncLaeuft}
          {syncVerbunden}
          {syncMeldung}
          {zuletztGeprueft}
          {protokoll}
          trockenlaufBauen={trockenlaufKoerper}
          trockenlaufSenden={trockenlaufSenden}
          teamBoard={daten.sync?.teamBoard ?? null}
          letzterAbgleich={daten.sync?.letzterAbgleich ?? null}
          {meineProjektIds}
          foerderungLabel={boardFoerderungLabel}
        />
      {:else if !aktivesProjekt}
        <div class="leer-projekt">
          <div class="karte">
            <h1>Noch kein Projekt erstellt</h1>
            <p class="untertitel">
              Matching und Merkliste gehören immer zu einem Projekt –
              mit eigenem Fragebogen und eigener Auswahl. Erstelle dein
              erstes Projekt, um loszulegen.
            </p>
            <button onclick={() => (neuesProjektOffen = true)}>
              Projekt erstellen
            </button>
          </div>
        </div>
      {:else if bereich === "passend"}
        {#key daten.aktivesProjektId}
          <Matching
            antworten={aktivesProjekt?.fragebogen ?? null}
            speichern={fragebogenSpeichern}
            merkliste={aktivesProjekt.merkliste}
            umschalten={merklisteUmschalten}
          />
        {/key}
      {:else if bereich === "formular"}
        {#key daten.aktivesProjektId}
          <SammelFormular
            formular={aktivesProjekt.formular}
            speichern={formularSpeichern}
            wordErzeugen={formularWordErzeugen}
          />
        {/key}
      {:else if bereich === "kostenplan"}
        {#key daten.aktivesProjektId}
          <KostenPlan
            kfp={aktivesProjekt.kfp}
            merkliste={aktivesProjekt.merkliste}
            foerderungen={alleFoerderungen}
            projektName={aktivesProjekt.name}
            speichern={kfpSpeichern}
            excelErzeugen={kfpExcelErzeugen}
            hinweisAusblenden={aktivesProjekt.kfpHinweisAusblenden}
            hinweisMerken={kfpHinweisMerken}
          />
        {/key}
      {:else if bereich === "fristen"}
        <Kalender
          foerderungen={alleFoerderungen}
          hinweis={katalog.daten.hinweis}
          merkliste={aktivesProjekt.merkliste}
          umschalten={merklisteUmschalten}
          {ordnerOeffnen}
          {dokumentHochladen}
          {antragsPdfVorschau}
          {antragsPdfSpeichern}
          antraege={aktivesProjekt.antraege}
          {antragHolen}
          antragSpeichern={tresorSpeichern}
          interneFristen={aktivesProjekt.interneFristen}
          interneAnlegen={interneFristAnlegen}
          interneEntfernen={interneFristEntfernen}
        />
      {:else}
        <Merkliste
          foerderungen={alleFoerderungen}
          hinweis={katalog.daten.hinweis}
          merkliste={aktivesProjekt.merkliste}
          umschalten={merklisteUmschalten}
          {ordnerOeffnen}
          {dokumentHochladen}
          {antragsPdfVorschau}
          {antragsPdfSpeichern}
          antraege={aktivesProjekt.antraege}
          {antragHolen}
          antragSpeichern={tresorSpeichern}
          eigeneAnlegen={eigeneFoerderungAnlegen}
        />
      {/if}
    </main>

    {#if sicherungOffen}
      <Sicherung schliessen={() => (sicherungOffen = false)} {nachWiederherstellung} />
    {/if}

    {#if neuesProjektOffen}
      <div class="schleier" onclick={() => (neuesProjektOffen = false)} role="presentation">
        <form class="karte" onsubmit={projektAnlegen} onclick={(e) => e.stopPropagation()}>
          <h1>Neues Projekt</h1>
          <label for="projektname">Name des Projekts</label>
          <input id="projektname" type="text" bind:value={neuerProjektName} />
          <button type="submit" disabled={!neuerProjektName.trim()}>Anlegen</button>
          <button type="button" class="leise" onclick={() => (neuesProjektOffen = false)}>
            Abbrechen
          </button>
        </form>
      </div>
    {/if}

    {#if umbenennenOffen}
      <div class="schleier" onclick={() => (umbenennenOffen = false)} role="presentation">
        <form class="karte" onsubmit={projektUmbenennen} onclick={(e) => e.stopPropagation()}>
          <h1>Projekt umbenennen</h1>
          <label for="umbenennen">Neuer Name</label>
          <input id="umbenennen" type="text" bind:value={umbenennenName} />
          <button type="submit" disabled={!umbenennenName.trim()}>Umbenennen</button>
          <button type="button" class="leise" onclick={() => (umbenennenOffen = false)}>
            Abbrechen
          </button>
        </form>
      </div>
    {/if}

    {#if loeschDialogOffen}
      <div class="schleier" onclick={() => (loeschDialogOffen = false)} role="presentation">
        <div class="karte" onclick={(e) => e.stopPropagation()} role="presentation">
          <h1>Projekt löschen?</h1>
          <p class="untertitel">
            Das Projekt <strong>„{aktivesProjekt?.name}"</strong> wird endgültig
            entfernt – samt seinem Fragebogen und Matching-Ergebnis.
            Das lässt sich nicht rückgängig machen.
          </p>
          <button class="gefahr" onclick={projektLoeschen}>Ja, löschen</button>
          <button class="leise" onclick={() => (loeschDialogOffen = false)}>
            Abbrechen
          </button>
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  :global(html, body) {
    margin: 0;
    height: 100%;
  }
  :global(body) {
    font-family: "Segoe UI", system-ui, sans-serif;
    background: #f4f5f7;
    color: #172b4d;
  }

  /* Zentrierte Bühne für die Passwort-Karten */
  .buehne {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 24px;
    box-sizing: border-box;
  }

  .karte {
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12), 0 8px 24px rgba(9, 30, 66, 0.08);
    padding: 40px;
    width: 100%;
    max-width: 380px;
    box-sizing: border-box;
  }
  h1 {
    margin: 0 0 4px;
    font-size: 1.5rem;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 1.2rem;
    font-weight: 600;
  }
  .untertitel {
    margin: 0 0 28px;
    color: #5e6c84;
    font-size: 0.95rem;
    line-height: 1.5;
  }
  .warntext {
    color: #ae2e24;
  }

  label {
    display: block;
    font-size: 0.85rem;
    font-weight: 600;
    color: #5e6c84;
    margin: 16px 0 6px;
  }

  input {
    width: 100%;
    box-sizing: border-box;
    padding: 10px 12px;
    font-size: 1rem;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
    transition: border-color 0.15s, background 0.15s;
  }
  input:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }

  button {
    width: 100%;
    margin-top: 20px;
    padding: 11px;
    font-size: 1rem;
    font-weight: 600;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s;
  }
  button:hover:not(:disabled) {
    background: #3d5bf0;
  }
  button:disabled {
    background: #c1c7d0;
    cursor: default;
  }

  button.gefahr {
    background: #ca3521;
  }
  button.gefahr:hover:not(:disabled) {
    background: #ae2e24;
  }

  button.leise {
    background: none;
    color: #5e6c84;
    font-weight: 400;
    font-size: 0.875rem;
    margin-top: 16px;
    padding: 6px;
  }
  button.leise:hover {
    background: none;
    color: #172b4d;
    text-decoration: underline;
  }

  .fehler {
    margin: 14px 0 0;
    color: #ae2e24;
    font-size: 0.9rem;
  }

  .warnung {
    margin: 24px 0 0;
    padding: 12px 14px;
    background: #fff7d6;
    border-radius: 8px;
    font-size: 0.85rem;
    line-height: 1.5;
    color: #533f04;
  }

  /* Ansicht nach dem Entsperren */
  .app header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 24px;
    background: #fff;
    box-shadow: 0 1px 2px rgba(9, 30, 66, 0.08);
  }
  .logo {
    font-weight: 700;
    font-size: 1.05rem;
  }
  .app header button.leise {
    width: auto;
    margin: 0;
  }
  .rechts {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  nav {
    display: flex;
    gap: 4px;
  }
  nav button {
    width: auto;
    margin: 0;
    padding: 7px 14px;
    font-size: 0.9rem;
    font-weight: 500;
    color: #44546f;
    background: none;
    border-radius: 8px;
  }
  nav button:hover:not(.aktiv) {
    background: #f1f2f4;
    color: #172b4d;
  }
  nav button.aktiv {
    background: #eef1ff;
    color: #3d5bf0;
    font-weight: 600;
  }
  /* trennt die drei Navigations-Blöcke */
  .nav-trenner {
    width: 1px;
    align-self: center;
    height: 22px;
    background: #dfe1e6;
    margin: 0 6px;
  }

  .links {
    display: flex;
    align-items: center;
    gap: 20px;
  }
  .projektwahl {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .projektwahl select {
    padding: 7px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    color: #172b4d;
    background: #f7f8f9;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    max-width: 220px;
  }
  .projektwahl select:focus {
    outline: none;
    border-color: #4f6df5;
  }
  .projektwahl button.leise {
    width: auto;
    margin: 0;
  }

  .schleier {
    position: fixed;
    inset: 0;
    background: rgba(9, 30, 66, 0.45);
    display: grid;
    place-items: center;
    padding: 24px;
    z-index: 20;
  }

  .leer-projekt {
    display: grid;
    place-items: center;
    padding: 64px 24px;
  }
</style>
