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
  import Abrechnung from "$lib/komponenten/Abrechnung.svelte";
  import Kostenstellen from "$lib/komponenten/Kostenstellen.svelte";
  import Geldquellen from "$lib/komponenten/Geldquellen.svelte";
  import Zuteilung from "$lib/komponenten/Zuteilung.svelte";
  import Sachbericht from "$lib/komponenten/Sachbericht.svelte";
  import Sicherung from "$lib/komponenten/Sicherung.svelte";
  import TeamSync from "$lib/komponenten/TeamSync.svelte";
  import { katalog, setzeKatalog, setzeStandardKatalog, standardKatalog, pruefeKatalog, vergleicheKataloge, geaenderteFelder, setzeGeteilteFoerderer } from "$lib/katalog.svelte.js";
  import KatalogUpdate from "$lib/komponenten/KatalogUpdate.svelte";
  import UpdatePruefung from "$lib/komponenten/UpdatePruefung.svelte";
  import { check as appUpdateCheck } from "@tauri-apps/plugin-updater";
  import { getVersion } from "@tauri-apps/api/app";
  import { formularWordBauen } from "$lib/antrag";
  import { antragsPdfBauen } from "$lib/antragsPdf";
  import { kfpExport, neuePostenId } from "$lib/kfp";
  import { verwendungsnachweisAbschnitte } from "$lib/abrechnung";
  import { ANTRAG_STANDARD, CHECK_STANDARD } from "$lib/status";
  import { boardAusTresor, geteilteFoerdererAusTresor } from "$lib/sync";
  // Reine Tresor-Struktur-Helfer (Vorlagen + Migration) – ausgelagert nach
  // $lib/tresorFormat.js, damit diese Datei kleiner bleibt.
  import { offizielleFristenAus, neueId, neuesProjekt, leereStammdaten, frischerTresor, normalisieren, STANDARD_TEAM_SYNC } from "$lib/tresorFormat.js";
  // Reine Katalog-Helfer (Team-Förderer umwandeln, Board-Label, Ghost-Pflege).
  import { teamFoerdererZuKatalog, boardFoerderungLabel, katalogGhostsAktualisieren } from "$lib/katalogHelfer.js";

  // Die App kennt fünf Ansichten:
  // laden -> einrichten (kein Tresor) ODER entsperren (Tresor da)
  // -> offen (entsperrt) | neu-aufsetzen (Passwort vergessen)
  let ansicht = $state("laden");
  // "Auf diesem Gerät merken" (passwortloses Entsperren per Windows DPAPI).
  // Nur auf Windows verfügbar; auf macOS/Linux wird die Option ausgeblendet.
  let merkenMoeglich = $state(false); // Plattform unterstützt DPAPI-Merken?
  let merkenAktiv = $state(false); // auf diesem Gerät bereits hinterlegt?
  let merkenWunsch = $state(false); // Häkchen auf dem Entsperr-Bildschirm

  let passwort = $state("");
  let passwortWdh = $state("");
  let fehler = $state("");
  let bestaetigung = $state("");
  let beschaeftigt = $state(false);

  // Die entschlüsselten Daten – leben nur im Arbeitsspeicher.
  let daten = $state(null);

  // Welcher Bereich ist nach dem Entsperren aktiv?
  let bereich = $state("foerderungen"); // foerderungen | merkliste | fristen | formular | kostenplan | belege | stammdaten
  // Oberster Arbeits-Modus: "antrag" (Recherche/Antrag) oder "abrechnung"
  // (Belege/Verwendungsnachweis). Blendet jeweils nur die passenden Reiter
  // ein, damit der andere Modus nicht visuell stört. Stammdaten ist in
  // beiden Modi erreichbar (globale Angaben).
  let arbeitsModus = $state("antrag");
  const ANTRAG_BEREICHE = ["foerderungen", "merkliste", "fristen", "formular", "kostenplan", "stammdaten"];
  const ABRECHNUNG_BEREICHE = ["belege", "kostenstellen", "geldquellen", "zuteilung", "sachbericht", "stammdaten"];
  function arbeitsModusWechseln(m) {
    arbeitsModus = m;
    const erlaubt = m === "abrechnung" ? ABRECHNUNG_BEREICHE : ANTRAG_BEREICHE;
    if (!erlaubt.includes(bereich)) {
      bereich = m === "abrechnung" ? "belege" : "foerderungen";
    }
  }
  let foerderAnsicht = $state("alle"); // innerhalb "Förderungen": alle | passend
  let projektMenuOffen = $state(false); // Projekt-Auswahlmenü (mit Umbenennen/Löschen)

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
  let katalogOffen = $state(false);
  let updateOffen = $state(false);
  let updateGeprueft = false; // Auto-Prüfung nur einmal pro Sitzung.
  // "Was ist neu?"-Meldung nach einem Update: { version, notes } oder null.
  let wasIstNeu = $state(null);

  // Datenbank-Förderungen, die vom Team geteilten eigenen Förderer und
  // die eigenen Förderungen des aktiven Projekts – diese Liste löst
  // überall die IDs auf.
  let alleFoerderungen = $derived([
    ...katalog.daten.foerderungen,
    ...katalog.geteilt,
    ...(aktivesProjekt?.eigeneFoerderungen ?? []),
    // Ehemalige Katalog-Förderungen, die ein Update entfernt hat, die du
    // aber noch gemerkt hast – bleiben als „nicht mehr im Katalog" sichtbar.
    ...(aktivesProjekt?.katalogGhosts ?? []),
  ]);

  // (teamFoerdererZuKatalog liegt jetzt in $lib/katalogHelfer.js.)

  // Hält den Katalog-Store mit den geteilten Team-Förderern synchron,
  // sobald der Sync neue holt (oder beim Entsperren aus dem Tresor).
  $effect(() => {
    setzeGeteilteFoerderer(teamFoerdererZuKatalog(daten?.sync?.teamFoerderer));
  });

  // Tresor-Vorlagen (frischerTresor/leereStammdaten/neuesProjekt/neueId),
  // offizielleFristenAus() und normalisieren() liegen jetzt in
  // $lib/tresorFormat.js (reine Helfer, siehe Import oben).

  onMount(async () => {
    // Beim Start eventuelle Klartext-Reste aus dem Temp-Ordner entfernen
    // (z. B. nach einem Absturz der vorigen Sitzung – Belege/PDF-Vorschau).
    invoke("temp_aufraeumen").catch(() => {});

    // Nutzer-Aktivitaet für den Idle-Lock erfassen (nur Zeitstempel setzen).
    for (const ev of ["mousemove", "keydown", "mousedown", "wheel", "touchstart"]) {
      window.addEventListener(ev, aktivitaetMerken, { passive: true });
    }

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

    // Unterstützt diese Plattform "Auf diesem Gerät merken"? (nur Windows)
    try {
      merkenMoeglich = await invoke("merken_moeglich");
    } catch {
      merkenMoeglich = false;
    }

    const status = await invoke("tresor_status");
    if (status === "fehlt") {
      ansicht = "einrichten";
      return;
    }
    // Tresor vorhanden: Gibt es ein gemerktes, passwortloses Entsperren?
    try {
      merkenAktiv = await invoke("merken_status");
    } catch {
      merkenAktiv = false;
    }
    merkenWunsch = merkenAktiv;
    if (merkenAktiv) {
      try {
        const json = await invoke("merken_entsperren");
        await nachEntsperren(json);
        return;
      } catch {
        // Gemerkter Zugang ungültig/entfernt → normales Entsperren.
        merkenAktiv = false;
        merkenWunsch = false;
      }
    }
    ansicht = "entsperren";
  });

  // Gemeinsamer Abschluss nach erfolgreichem Entsperren (egal ob per
  // Passwort oder per gemerktem Zugang).
  async function nachEntsperren(json) {
    daten = JSON.parse(json);
    // Ältere Datenstände (z. B. aus Schritt 3) sanft überführen.
    if (normalisieren(daten)) await tresorSpeichern();
    passwort = "";
    ansicht = "offen";
    // Server-Erreichbarkeit einmal prüfen, damit der Status-Punkt stimmt.
    if (daten?.sync && online) verbindungPruefen().catch(() => {});
    // Nach einem gerade installierten Update einmalig "Was ist neu?" zeigen.
    await wasIstNeuPruefen();
    // Einmal still nach einer neuen App-Version schauen (Etappe 5).
    updateStillPruefen();
    // Und die Förder-Datenbank automatisch auf Aktualisierungen prüfen.
    katalogStillPruefen();
  }

  // Ein-Klick-Entsperren über den gemerkten Zugang (auf dem Sperr-Bildschirm).
  async function merkenEntsperrenJetzt() {
    fehler = "";
    beschaeftigt = true;
    try {
      const json = await invoke("merken_entsperren");
      await nachEntsperren(json);
    } catch {
      merkenAktiv = false;
      fehler = "Automatisches Entsperren nicht möglich – bitte Passwort eingeben.";
    } finally {
      beschaeftigt = false;
    }
  }

  // Den gemerkten Zugang dieses Geräts wieder entfernen.
  async function merkenVergessen() {
    try {
      await invoke("merken_vergessen");
    } catch { /* nicht fatal */ }
    merkenAktiv = false;
    merkenWunsch = false;
  }

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
      // Erstes Aktivieren: die eingebackene Server-Anbindung sofort und
      // automatisch herstellen – Förder-Datenbank im Hintergrund holen und
      // still nach einer neueren App-Version schauen. Kein Knopfdruck nötig.
      // (Beim späteren Entsperren erledigt das nachEntsperren; hier fehlte es.)
      katalogStillPruefen();
      updateStillPruefen();
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
      // Merken-Wunsch umsetzen (Tresor ist jetzt offen → Schlüssel vorhanden).
      try {
        if (merkenWunsch) {
          await invoke("merken_anlegen");
          merkenAktiv = true;
        } else if (merkenAktiv) {
          await invoke("merken_vergessen");
          merkenAktiv = false;
        }
      } catch { /* Merken ist Komfort, kein harter Fehler */ }
      await nachEntsperren(json);
    } catch (e) {
      fehler =
        String(e) === "falsches_passwort" ? "Falsches Passwort." : String(e);
      passwort = "";
    } finally {
      beschaeftigt = false;
    }
  }

  // --- Idle-Lock (Audit I3): sperrt den Tresor nach Inaktivitaet, damit ein
  //     unbeaufsichtigtes, entsperrtes Geraet nicht offen stehen bleibt.
  //     Aktivitaet aktualisiert nur einen Zeitstempel (billig); ein Intervall
  //     prueft minuetlich, ob die Leerlauf-Grenze ueberschritten ist. ---
  const IDLE_MS = 30 * 60 * 1000; // 30 Minuten ohne Aktivitaet -> sperren
  let letzteAktivitaet = Date.now();
  let idleInterval = null;
  function aktivitaetMerken() {
    letzteAktivitaet = Date.now();
  }
  $effect(() => {
    if (daten) {
      letzteAktivitaet = Date.now();
      if (!idleInterval) {
        idleInterval = setInterval(() => {
          if (daten && Date.now() - letzteAktivitaet >= IDLE_MS) sperren();
        }, 60 * 1000);
      }
    } else if (idleInterval) {
      clearInterval(idleInterval);
      idleInterval = null;
    }
  });

  async function sperren() {
    syncLoopStoppen();
    syncVerbunden = false;
    syncMeldung = null;
    zuletztGeprueft = null;
    await invoke("tresor_sperren");
    // Beim Sperren die kurzlebigen Klartext-Reste (entschluesselte Belege,
    // Antrags-PDF-Vorschau) aus dem Temp-Ordner entfernen.
    invoke("temp_aufraeumen").catch(() => {});
    daten = null;
    fehler = "";
    bereich = "foerderungen";
    arbeitsModus = "antrag";
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

  // Belege des aktiven Projekts ersetzen und verschlüsselt sichern.
  // (Abrechnungs-Modus, Phase A1. Quellen folgen in Phase A4.)
  async function belegeSpeichern(neueBelege) {
    aktivesProjekt.abrechnung.belege = neueBelege;
    await tresorSpeichern();
  }

  // Projektweiten Sachbericht speichern.
  async function sachberichtSpeichern(text) {
    aktivesProjekt.abrechnung.sachbericht = text;
    await tresorSpeichern();
  }

  // Verwendungsnachweis einer Geldquelle als PDF bzw. Word erzeugen
  // (Phase A5). Baut die Abschnitte im Frontend und lässt Rust rendern.
  async function nachweisExport(quelleId, format) {
    const a = aktivesProjekt.abrechnung;
    const q = a.quellen.find((x) => x.id === quelleId);
    if (!q) return;
    const { titel, abschnitte } = verwendungsnachweisAbschnitte(
      q,
      a.belege,
      aktivesProjekt.kfp,
      aktivesProjekt.name,
      a.sachbericht
    );
    const cmd = format === "word" ? "verwendungsnachweis_word" : "verwendungsnachweis_pdf";
    try {
      await invoke(cmd, {
        projekt: aktivesProjekt.name,
        foerderer: q.name,
        titel,
        abschnitte,
        logo: daten.stammdaten?.logo || null,
      });
    } catch (e) {
      alert("Der Verwendungsnachweis konnte nicht erzeugt werden.\n" + e);
    }
  }

  // Geldquellen des aktiven Projekts ersetzen und sichern (Phase A4).
  async function quellenSpeichern(neueQuellen) {
    aktivesProjekt.abrechnung.quellen = neueQuellen;
    await tresorSpeichern();
  }

  // Eine Geldquelle entfernen UND ihre Zuordnungen aus allen Belegen
  // herausnehmen (sonst zeigten Belege auf eine nicht mehr existierende
  // Quelle).
  async function quelleEntfernen(quelleId) {
    const a = aktivesProjekt.abrechnung;
    a.quellen = a.quellen.filter((q) => q.id !== quelleId);
    for (const b of a.belege) {
      if (Array.isArray(b.zuordnungen)) {
        b.zuordnungen = b.zuordnungen.filter((z) => z.quelleId !== quelleId);
      }
    }
    await tresorSpeichern();
  }

  // Neue Kostenstelle (KFP-Kosten-Posten) aus der Beleg-Maske anlegen.
  // Gibt die neue Posten-ID zurück, die die Beleg-Maske gleich auswählt.
  async function kostenstelleAnlegen(kategorieIndex, bezeichnung) {
    const kat = aktivesProjekt.kfp?.kosten?.[kategorieIndex];
    if (!kat) return null;
    const id = neuePostenId();
    if (!Array.isArray(kat.posten)) kat.posten = [];
    kat.posten.push({ id, bezeichnung: bezeichnung.trim(), erlaeuterung: "", betrag: "" });
    await tresorSpeichern();
    return id;
  }

  // --- Beleg-Dateien (Phase A2): verschlüsselt im Projektordner ---
  // Datei wählen und verschlüsselt ablegen. Gibt den Verweis
  // { ref, name, ext, groesse } zurück, den die Komponente am Beleg merkt.
  async function belegDateiHinzufuegen(belegId) {
    const pfad = await dateiWaehlen({
      title: "Beleg auswählen (PDF oder Bild)",
      multiple: false,
      filters: [{ name: "PDF oder Bild", extensions: ["pdf", "jpg", "jpeg", "png"] }],
    });
    if (!pfad) return null; // abgebrochen
    try {
      return await invoke("beleg_datei_hinzufuegen", {
        projekt: aktivesProjekt.name,
        belegId,
        quelle: pfad,
      });
    } catch (e) {
      alert("Die Datei konnte nicht hinzugefügt werden.\n" + e);
      return null;
    }
  }

  async function belegDateiOeffnen(belegId, dateiRef, name) {
    try {
      await invoke("beleg_datei_oeffnen", {
        projekt: aktivesProjekt.name,
        belegId,
        dateiRef,
        name,
      });
    } catch (e) {
      alert("Die Datei konnte nicht geöffnet werden.\n" + e);
    }
  }

  // Eine Beleg-Datei entschlüsselt an einen selbst gewählten Ort speichern.
  async function belegDateiHerunterladen(belegId, dateiRef, name) {
    const ziel = await dateiSpeichern({
      title: "Beleg speichern unter",
      defaultPath: name,
    });
    if (!ziel) return; // abgebrochen
    try {
      await invoke("beleg_datei_exportieren", {
        projekt: aktivesProjekt.name,
        belegId,
        dateiRef,
        ziel,
      });
    } catch (e) {
      alert("Die Datei konnte nicht gespeichert werden.\n" + e);
    }
  }

  async function belegDateiEntfernen(belegId, dateiRef) {
    await invoke("beleg_datei_entfernen", {
      projekt: aktivesProjekt.name,
      belegId,
      dateiRef,
    });
  }

  // Beim Löschen eines Belegs auch seinen Datei-Ordner entfernen.
  async function belegOrdnerEntfernen(belegId) {
    try {
      await invoke("beleg_ordner_entfernen", {
        projekt: aktivesProjekt.name,
        belegId,
      });
    } catch {
      // Nicht kritisch: bleibt ein verwaister, verschlüsselter Ordner.
    }
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
      logo: daten.stammdaten?.logo || null,
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
    return { projekt: aktivesProjekt.name, foerderung: foerderung.name, titel, abschnitte, anhaenge, logo: daten.stammdaten?.logo || null };
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
        caPem: info.ca_pem ?? "",
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
        caPem: daten.sync.caPem ?? "",
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

  // --- Gehostetes Modell (Schritt 4b): Einladung annehmen / Mitglied einladen ---
  // Neues Gerät: Einladungs-Datei wählen und kopiersicher verbinden. Der
  // Geräteschlüssel entsteht dabei LOKAL (in Rust) und verlässt das Gerät nie;
  // gesendet wird nur der öffentliche Teil, zurück kommt der signierte Ausweis.
  async function einladungAnnehmen(geraetName) {
    const name = (geraetName || "").trim();
    if (!name) return null;
    const pfad = await dateiWaehlen({
      title: "Einladung wählen",
      multiple: false,
      filters: [{ name: "Einladung", extensions: ["a3keinladung", "json"] }],
    });
    if (!pfad) return null;
    try {
      const einl = await invoke("einladung_lesen", { pfad });
      const info = await invoke("enroll_annehmen", {
        enrollUrl: einl.enroll_url,
        syncAdresse: einl.sync_adresse,
        caPem: einl.ca_pem ?? "",
        token: einl.token,
        geraetName: name,
      });
      daten.sync = {
        adresse: info.adresse,
        geraetName: info.geraet_name,
        ausweisPem: info.ausweis_pem,
        caPem: info.ca_pem ?? "",
        letzterAbgleich: null,
      };
      await tresorSpeichern();
      return info;
    } catch (e) {
      alert("Die Einladung konnte nicht angenommen werden.\n" + e);
      return null;
    }
  }

  // Eigentümer: eine Einladung für ein weiteres Gerät erstellen und als Datei
  // speichern (offline weitergeben – der Einmal-Code ist wie ein Schlüssel).
  async function mitgliedEinladen(enrollUrl, geraetName) {
    if (!daten.sync) return null;
    const url = (enrollUrl || "").trim();
    if (!url) {
      alert("Bitte die öffentliche Verbindungs-Adresse angeben (z. B. https://sync.antrag3000.de).");
      return null;
    }
    const sicher = ((geraetName || "").trim().replace(/[^A-Za-z0-9_.-]/g, "_")) || "einladung";
    const ziel = await dateiSpeichern({
      title: "Einladung speichern",
      defaultPath: sicher + ".a3keinladung",
      filters: [{ name: "Einladung", extensions: ["a3keinladung"] }],
    });
    if (!ziel) return null;
    try {
      const erg = await invoke("einladung_erstellen", {
        syncAdresse: daten.sync.adresse,
        ausweisPem: daten.sync.ausweisPem,
        caPem: daten.sync.caPem ?? "",
        enrollUrl: url,
        geraetName: (geraetName || "").trim(),
        ziel,
      });
      alert(
        "Einladung gespeichert:\n" + ziel +
        "\n\nGib die Datei offline an das neue Gerät weiter (z. B. USB) – nicht per Mail." +
        "\nSie ist nur EINMAL nutzbar" + (erg?.ablauf ? " und läuft am " + erg.ablauf + " ab." : "."),
      );
      return erg;
    } catch (e) {
      alert("Die Einladung konnte nicht erstellt werden.\n" + e);
      return null;
    }
  }

  // Eigenes Team erstellen (gehostet, ohne Konto/E-Mail): erzeugt lokal den
  // Schlüssel, bootstrappt am Server ein neues Konto und wird dessen Eigentümer.
  async function teamErstellen(teamUrl, syncAdresse, caPem, teamName, geraetName) {
    try {
      const info = await invoke("team_erstellen", {
        teamUrl,
        syncAdresse,
        caPem: caPem ?? "",
        teamName,
        geraetName,
      });
      daten.sync = {
        adresse: info.adresse,
        geraetName: info.geraet_name,
        ausweisPem: info.ausweis_pem,
        caPem: info.ca_pem ?? "",
        letzterAbgleich: null,
      };
      await tresorSpeichern();
      return info;
    } catch (e) {
      alert("Das Team konnte nicht erstellt werden.\n" + e);
      return null;
    }
  }

  // Eigentümer: die Team-Geräte auflisten bzw. sperren/entsperren.
  async function mitgliederHolen() {
    if (!daten.sync) return null;
    try {
      const roh = await invoke("mitglieder_holen", {
        syncAdresse: daten.sync.adresse,
        ausweisPem: daten.sync.ausweisPem,
        caPem: daten.sync.caPem ?? "",
      });
      return JSON.parse(roh);
    } catch (e) {
      alert("Die Mitglieder konnten nicht geladen werden.\n" + e);
      return null;
    }
  }
  async function mitgliedStatusSetzen(geraetId, status) {
    if (!daten.sync) return false;
    try {
      await invoke("mitglied_status_setzen", {
        syncAdresse: daten.sync.adresse,
        ausweisPem: daten.sync.ausweisPem,
        caPem: daten.sync.caPem ?? "",
        geraetId,
        status,
      });
      return true;
    } catch (e) {
      alert("Die Änderung war nicht möglich.\n" + e);
      return false;
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

  // --- Statusanzeige im Header (kleiner farbiger Punkt) ---
  // rot: offline · blau: online, aber kein Team eingerichtet ·
  // orange: online, Team eingerichtet, Server zurzeit nicht erreichbar ·
  // grün: online und mit dem Team verbunden.
  let online = $state(typeof navigator !== "undefined" ? navigator.onLine : true);
  $effect(() => {
    if (typeof window === "undefined") return;
    const auf = () => {
      online = true;
      // Wieder im Netz: Server-Erreichbarkeit erneut prüfen (für den Punkt).
      if (daten?.sync && !syncLaeuft) verbindungPruefen().catch(() => {});
    };
    const ab = () => {
      online = false;
      syncVerbunden = false;
    };
    window.addEventListener("online", auf);
    window.addEventListener("offline", ab);
    return () => {
      window.removeEventListener("online", auf);
      window.removeEventListener("offline", ab);
    };
  });

  let verbindungsStatus = $derived(
    !online
      ? { klasse: "rot", text: "Offline – keine Netzwerkverbindung" }
      : daten?.modus === "einzel"
        ? { klasse: "blau", text: "Einzelplatz – online (Updates verfügbar)" }
        : !daten?.sync
          ? { klasse: "blau", text: "Online, nicht mit einem Team verbunden" }
          : syncVerbunden
            ? { klasse: "gruen", text: "Online und mit dem Team verbunden" }
            : { klasse: "orange", text: "Online, Server zurzeit nicht erreichbar" }
  );

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

  // Etappe 5: stille Update-Prüfung beim Start. Bei einer gefundenen,
  // gültig signierten neuen Version öffnet sich der Update-Dialog; jeder
  // Fehler (z. B. Server nicht erreichbar) wird bewusst verschluckt.
  // Vor dem Neustart (aus dem Update-Dialog) merken, was in der neuen Version
  // neu ist. Wird verschlüsselt im Tresor abgelegt und beim nächsten Start
  // einmalig angezeigt.
  async function updateHinweisMerken(version, notes) {
    if (!daten) return;
    daten.updateHinweis = { version, notes: notes || "" };
    await tresorSpeichern();
  }

  // Beim Öffnen: Gibt es einen gemerkten Update-Hinweis, der zur JETZT
  // laufenden Version passt? Dann einmalig "Was ist neu?" zeigen. Danach
  // (oder wenn veraltet) den Hinweis entfernen, damit er nicht wiederkehrt.
  async function wasIstNeuPruefen() {
    if (!daten?.updateHinweis) return;
    let aktuell = "";
    try { aktuell = await getVersion(); } catch { return; }
    if (daten.updateHinweis.version === aktuell) {
      wasIstNeu = { version: aktuell, notes: daten.updateHinweis.notes || "" };
    }
    delete daten.updateHinweis;
    await tresorSpeichern();
  }

  async function updateStillPruefen() {
    if (updateGeprueft) return;
    updateGeprueft = true;
    try {
      const u = await appUpdateCheck();
      if (u) {
        try { await u.close(); } catch { /* Handle freigeben */ }
        updateOffen = true; // Dialog prüft selbst erneut und zeigt Details.
      }
    } catch {
      /* still: kein Update-Server erreichbar o. Ä. */
    }
  }

  // Beim Start die Förder-Datenbank im Hintergrund auf Aktualisierungen
  // prüfen (nur wenn ein Server eingerichtet ist). Änderungen werden über die
  // „NEU"-Markierungen und den Merklisten-Hinweis sichtbar. Offline-first:
  // ist der Server nicht erreichbar, bleibt der lokale Katalog unangetastet.
  let katalogGeprueft = false;
  async function katalogStillPruefen() {
    if (katalogGeprueft || !katalogServerBereit) return;
    katalogGeprueft = true;
    try {
      await katalogUpdateVomServer();
    } catch {
      /* still: Server nicht erreichbar */
    }
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
    const caPem = daten.sync.caPem ?? "";
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
      await invoke("sync_delete_board", { adresse, ausweisPem, caPem, projektId: id });
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
        adresse, ausweisPem, caPem, projektId: p.id, bodyJson: body,
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
    const boardText = await invoke("sync_get_board", { adresse, ausweisPem, caPem });
    const serverBoard = JSON.parse(boardText);
    for (const row of serverBoard) versionen[row.projekt_id] = row.version;
    if (JSON.stringify(daten.sync.teamBoard ?? null) !== JSON.stringify(serverBoard)) {
      daten.sync.teamBoard = serverBoard;
      geaendert = true;
    }

    // 4. Offene Katalog-Meldungen senden (Upsert per id auf dem Server).
    //    Schlägt eine Meldung fehl (z. B. Tempo-Bremse 429), wird sie
    //    einfach beim nächsten Takt erneut versucht – nicht abbrechen.
    for (const m of daten.katalogMeldungen ?? []) {
      if (m.gesendet) continue;
      const body = JSON.stringify({
        foerderungId: m.foerderungId,
        foerderungName: m.foerderungName,
        art: m.art,
        text: m.text,
      });
      try {
        await invoke("sync_meldung_senden", {
          adresse, ausweisPem, caPem, meldungId: m.id, bodyJson: body,
        });
        m.gesendet = true;
        geaendert = true;
        protZeilen.push({
          projektId: `Meldung: ${m.foerderungName || m.foerderungId}`,
          aktion: "Meldung gesendet",
          bytes: body.length,
          body,
        });
      } catch (e) {
        // Fehler (Netz/Tempo-Bremse) protokollieren, aber weitermachen.
        protZeilen.push({
          projektId: `Meldung: ${m.foerderungName || m.foerderungId}`,
          aktion: `nicht gesendet (${e})`,
          bytes: 0,
          body: null,
        });
      }
    }

    // 5. Eigene Förderer teilen (nur öffentliche Felder; Quelle ist
    //    allein geteilteFoerdererAusTresor – die Beschreibung bleibt lokal).
    const geteilt = geteilteFoerdererAusTresor($state.snapshot(daten));
    const gesendetF = { ...(daten.sync.gesendetFoerderer ?? {}) };
    const aktuelleFIds = new Set(geteilt.foerderer.map((f) => f.id));

    // 5a. Zurückziehen: früher geteilte, die es lokal nicht mehr gibt.
    for (const id of Object.keys(gesendetF)) {
      if (aktuelleFIds.has(id)) continue;
      try {
        await invoke("sync_foerderer_loeschen", { adresse, ausweisPem, caPem, foerdererId: id });
        delete gesendetF[id];
        protZeilen.push({ projektId: `Förderer zurückgezogen: ${id}`, aktion: "gelöscht", bytes: 0, body: null });
        geaendert = true;
      } catch (e) { /* nächster Takt erneut */ }
    }

    // 5b. Teilen – nur, wenn sich die öffentliche Sicht geändert hat.
    for (const f of geteilt.foerderer) {
      const js = JSON.stringify(f.inhalt);
      if (gesendetF[f.id] === js) continue;
      const body = JSON.stringify({ inhalt: f.inhalt });
      try {
        await invoke("sync_foerderer_senden", {
          adresse, ausweisPem, caPem, foerdererId: f.id, bodyJson: body,
        });
        gesendetF[f.id] = js;
        geaendert = true;
        protZeilen.push({
          projektId: `Förderer geteilt: ${f.inhalt.name || f.id}`,
          aktion: "gesendet", bytes: body.length, body,
        });
      } catch (e) {
        protZeilen.push({
          projektId: `Förderer: ${f.inhalt.name || f.id}`,
          aktion: `nicht geteilt (${e})`, bytes: 0, body: null,
        });
      }
    }
    if (JSON.stringify(daten.sync.gesendetFoerderer ?? {}) !== JSON.stringify(gesendetF)) {
      daten.sync.gesendetFoerderer = gesendetF;
      geaendert = true;
    }

    // 6. Team-Förderer holen. Eigene (lokal vorhandene) ids ausschließen,
    //    damit man sich nicht selbst doppelt im Katalog sieht.
    const foerdererText = await invoke("sync_foerderer_holen", { adresse, ausweisPem, caPem });
    const serverFoerderer = JSON.parse(foerdererText);
    const eigeneIds = new Set();
    for (const p of daten.projekte) {
      for (const e of p.eigeneFoerderungen ?? []) eigeneIds.add(e.id);
    }
    const fremde = serverFoerderer.filter((r) => !eigeneIds.has(r.id));
    if (JSON.stringify(daten.sync.teamFoerderer ?? null) !== JSON.stringify(fremde)) {
      daten.sync.teamFoerderer = fremde;
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
  // ohne irgendetwas zu senden. Quellen sind allein boardAusTresor() und
  // geteilteFoerdererAusTresor() (dieselben Stellen wie im echten Sync).
  function trockenlaufKoerper() {
    const schnapp = $state.snapshot(daten ?? {});
    const board = boardAusTresor(schnapp);
    const projektKoerper = board.projekte.map((p) =>
      JSON.stringify({ inhalt: p, basis_version: null }, null, 2),
    );
    const geteilt = geteilteFoerdererAusTresor(schnapp);
    const foerdererKoerper = geteilt.foerderer.map((f) =>
      JSON.stringify({ inhalt: f.inhalt }, null, 2),
    );
    return [...projektKoerper, ...foerdererKoerper];
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

  // (boardFoerderungLabel liegt jetzt in $lib/katalogHelfer.js.)

  // --- Phase 3 / Etappe 2+3: Katalog-Update (Datei ODER Team-Server) ---
  // Gemeinsame Anwende-Logik: prüfen, automatisch übernehmen, Vergleich +
  // Feld-Diff + „zuletzt aktualisiert"-Stand merken. Quelle ist egal –
  // eine lokale Datei (Pilot) oder die NAS (Etappe 3).
  async function katalogUpdateAnwenden(obj, quelle = "datei") {
    const p = pruefeKatalog(obj);
    if (!p.ok) return { ok: false, fehler: p.fehler };

    const altKatalog = katalog.daten.foerderungen;
    const diff = vergleicheKataloge(altKatalog, obj.foerderungen);
    try {
      await invoke("katalog_speichern", { inhalt: JSON.stringify(obj) });
      katalogGhostsAktualisieren(daten, altKatalog, obj.foerderungen);
      // Gemerkte Förderungen vormerken, deren Angaben sich geändert haben –
      // für den Hinweis oben auf der Merkliste.
      const geaendertIds = new Set(diff.geaendert.map((e) => e.id));
      for (const projekt of daten.projekte ?? []) {
        const betroffen = (projekt.merkliste ?? []).filter((id) => geaendertIds.has(id));
        if (betroffen.length) {
          const bisher = new Set(projekt.katalogAktualisiert ?? []);
          for (const id of betroffen) bisher.add(id);
          projekt.katalogAktualisiert = [...bisher];
        }
      }
      // Pro-Förderung den „zuletzt aktualisiert"-Stand merken (neu + geändert).
      if (!daten.katalogStand) daten.katalogStand = {};
      for (const e of [...diff.neu, ...diff.geaendert]) {
        daten.katalogStand[e.id] = obj.stand;
      }
      // Welche FELDER sich je geänderter Förderung geändert haben –
      // für die „NEU"-Markierung am konkreten Feld (z. B. Förderhöhe).
      if (!daten.katalogFeldDiff) daten.katalogFeldDiff = {};
      const altMap = new Map(altKatalog.map((f) => [f.id, f]));
      const neuMap = new Map(obj.foerderungen.map((f) => [f.id, f]));
      for (const e of diff.geaendert) {
        daten.katalogFeldDiff[e.id] = geaenderteFelder(altMap.get(e.id), neuMap.get(e.id));
      }
      setzeKatalog(obj, quelle);
      await tresorSpeichern();
      return { ok: true, diff, stand: obj.stand };
    } catch (e) {
      return { ok: false, fehler: "Konnte nicht gespeichert werden: " + e };
    }
  }

  // Update aus einer lokalen Datei (Pilot/Test).
  async function katalogUpdateAusDatei() {
    const pfad = await dateiWaehlen({
      title: "Förder-Katalog-Update wählen",
      multiple: false,
      filters: [{ name: "Katalog (JSON)", extensions: ["json"] }],
    });
    if (!pfad) return null;
    let obj;
    try {
      const roh = await invoke("katalog_kandidat_lesen", { pfad });
      obj = JSON.parse(roh);
    } catch (e) {
      return { ok: false, fehler: "Datei nicht lesbar oder kein gültiges JSON." };
    }
    return katalogUpdateAnwenden(obj, "datei");
  }

  // Update vom Team-Server holen (mTLS) – Etappe 3. Braucht ein
  // Lädt das VOLLE Förderer-Logo von der NAS (über denselben Weg wie der
  // Katalog: Team = mTLS, Einzelplatz = offener Kanal) und speichert es per
  // Dialog als Bilddatei. Die kleine Kachel-Vorschau steckt schon im Katalog.
  async function logoHerunterladen(f) {
    if (!f?.logo_id) return;
    try {
      let datenUrl = null;
      if (daten.modus === "team" && daten.sync) {
        datenUrl = await invoke("sync_logo_holen", {
          adresse: daten.sync.adresse,
          ausweisPem: daten.sync.ausweisPem,
          caPem: daten.sync.caPem ?? "",
          logoId: f.logo_id,
        });
      } else if (daten.einzelServer) {
        datenUrl = await invoke("logo_oeffentlich_holen", {
          adresse: daten.einzelServer,
          logoId: f.logo_id,
        });
      } else {
        alert("Kein Server eingerichtet, um das Logo zu laden.");
        return;
      }
      if (!datenUrl) {
        alert("Für diesen Förderer ist (noch) kein Logo hinterlegt.");
        return;
      }
      await invoke("logo_herunterladen", {
        datenUrl,
        vorschlagName: "Logo " + (f.foerdergeber || f.name || "Foerderer"),
      });
    } catch (e) {
      alert("Logo konnte nicht geladen werden:\n" + e);
    }
  }

  // eingerichtetes Gerät (Zugangs-Paket geladen).
  async function katalogUpdateVomServer() {
    let roh;
    try {
      if (daten.modus === "team" && daten.sync) {
        // Team: über die mTLS-Verbindung (Zugangs-Paket).
        roh = await invoke("sync_katalog_holen", {
          adresse: daten.sync.adresse,
          ausweisPem: daten.sync.ausweisPem,
          caPem: daten.sync.caPem ?? "",
        });
      } else if (daten.einzelServer) {
        // Einzelplatz: über den offenen Kanal, ohne Zertifikat.
        roh = await invoke("katalog_oeffentlich_holen", { adresse: daten.einzelServer });
      } else {
        return { ok: false, fehler: "Kein Server eingerichtet." };
      }
    } catch (e) {
      return { ok: false, fehler: "Vom Server nicht ladbar: " + e };
    }
    let obj;
    try {
      obj = JSON.parse(roh);
    } catch {
      return { ok: false, fehler: "Server-Antwort ist kein gültiger Katalog." };
    }
    return katalogUpdateAnwenden(obj, "server");
  }

  // Ist ein Server für den Katalog-Abruf bereit (je nach Modus)?
  let katalogServerBereit = $derived(
    (daten?.modus === "team" && !!daten?.sync) ||
      (daten?.modus === "einzel" && !!daten?.einzelServer),
  );

  // Einzelplatz: Modus wechseln bzw. Katalog manuell holen.
  let einzelMeldung = $state("");
  // Self-Hosting-Override: die Server-Adresse ist eingebacken und normalerweise
  // unsichtbar; nur wer einen eigenen Server betreibt, klappt sie hier auf.
  let selfHostOffen = $state(false);
  async function modusWechseln(m) {
    daten.modus = m;
    if (m === "einzel") autoSyncStoppen();
    einzelMeldung = "";
    await tresorSpeichern();
  }
  async function einzelKatalogHolen() {
    einzelMeldung = "Wird geprüft …";
    const r = await katalogUpdateVomServer();
    if (r?.ok) {
      const n = (r.diff?.neu?.length ?? 0) + (r.diff?.geaendert?.length ?? 0);
      einzelMeldung = n > 0 ? `✓ Katalog aktualisiert (${n} Änderungen).` : "✓ Katalog ist aktuell.";
    } else {
      einzelMeldung = "⚠ " + (r?.fehler ?? "Konnte nicht laden.");
    }
  }

  // „Zuletzt aktualisiert"-Datum für eine Förderung (formatiert) oder null.
  // Bekannter Pro-Eintrag-Stand, sonst der globale Katalog-Stand (nur für
  // echte Katalog-Einträge; eigene/Ghost-Einträge bekommen keins).
  function katalogStandFuer(id) {
    if (!daten) return null;
    const iso = daten.katalogStand?.[id]
      ?? (katalog.daten.foerderungen.some((f) => f.id === id) ? katalog.daten.stand : null);
    if (!iso) return null;
    const d = new Date(iso);
    return isNaN(d) ? iso : d.toLocaleDateString("de-CH");
  }

  // Welche Felder einer Förderung als „NEU" markiert werden sollen:
  // nur, solange die Förderung im aktiven Projekt als „aktualisiert" gilt
  // (also bis „OK, verstanden"). Liste der geänderten Feld-Schlüssel.
  function katalogNeuFelder(id) {
    if (!aktivesProjekt) return [];
    if (!(aktivesProjekt.katalogAktualisiert ?? []).includes(id)) return [];
    return daten?.katalogFeldDiff?.[id] ?? [];
  }

  // Den „aktualisiert"-Hinweis des aktiven Projekts wegklicken.
  async function katalogHinweisVerwerfen() {
    if (aktivesProjekt) {
      aktivesProjekt.katalogAktualisiert = [];
      await tresorSpeichern();
    }
  }

  // (katalogGhostsAktualisieren liegt jetzt in $lib/katalogHelfer.js und
  //  bekommt `daten` als ersten Parameter übergeben.)

  async function katalogZuruecksetzen() {
    try {
      await invoke("katalog_zuruecksetzen");
      katalogGhostsAktualisieren(daten, katalog.daten.foerderungen, standardKatalog().foerderungen);
      for (const projekt of daten.projekte ?? []) projekt.katalogAktualisiert = [];
      daten.katalogStand = {};
      daten.katalogFeldDiff = {};
      setzeStandardKatalog();
      await tresorSpeichern();
      return { ok: true };
    } catch (e) {
      return { ok: false, fehler: String(e) };
    }
  }

  // Eine Melde-Warteschlange im Tresor (Versand an den Server kommt mit
  // der NAS). Client-Schutz: Grund-Pflicht, Längenlimit, keine doppelte
  // offene Meldung zur selben Förderung + Art.
  const MELDUNG_MAX = 500;
  function meldungAnlegen(foerderungId, foerderungName, art, text) {
    const t = (text ?? "").trim().slice(0, MELDUNG_MAX);
    if (!foerderungId || !art) return { ok: false, fehler: "Bitte Förderung und Art angeben." };
    if ((daten.katalogMeldungen ?? []).length >= MELDUNG_CAP) {
      return { ok: false, fehler: `Es warten schon viele Meldungen (Grenze ${MELDUNG_CAP}). Bitte zuerst senden/aufräumen.` };
    }
    const offen = (daten.katalogMeldungen ?? []).some(
      (m) => !m.gesendet && m.foerderungId === foerderungId && m.art === art,
    );
    if (offen) return { ok: false, fehler: "Dazu gibt es schon eine offene Meldung." };
    daten.katalogMeldungen = [
      {
        id: neueId(),
        foerderungId,
        foerderungName: (foerderungName ?? "").trim(),
        art,
        text: t,
        zeit: new Date().toISOString(),
        gesendet: false,
      },
      ...(daten.katalogMeldungen ?? []),
    ];
    tresorSpeichern();
    return { ok: true };
  }
  function meldungEntfernen(id) {
    daten.katalogMeldungen = (daten.katalogMeldungen ?? []).filter((m) => m.id !== id);
    tresorSpeichern();
  }

  // Liefert (und erstellt bei Bedarf) den Antrag-Status-Eintrag einer
  // gemerkten Förderung. Die Checkliste startet mit den üblichen
  // Unterlagen der Förderung.
  function antragHolen(foerderung) {
    if (!aktivesProjekt.antraege[foerderung.id]) {
      aktivesProjekt.antraege[foerderung.id] = {
        status: ANTRAG_STANDARD,
        statusFrei: "",
        offizielleFristen: offizielleFristenAus(foerderung),
        eigeneFristen: [],
        kontakt: { ansprechpartner: "", email: "", telefon: "", notiz: "" },
        checkliste: (foerderung.checkliste_vorschlag ?? []).map((t) => ({
          text: t,
          status: CHECK_STANDARD,
          statusFrei: "",
          datei: "",
        })),
      };
    }
    // WICHTIG: immer den reaktiven $state-Proxy verwenden – NICHT das frisch
    // erzeugte Roh-Objekt. Sonst lösen Änderungen in der Detailansicht
    // (z. B. ein Dokument entfernen) keine Aktualisierung der Listenansicht
    // und des PDF-Knopfs aus.
    const a = aktivesProjekt.antraege[foerderung.id];
    // Offizielle Frist(en): aus der Datenbank vorbefüllt, aber editierbar.
    if (!Array.isArray(a.offizielleFristen)) {
      a.offizielleFristen = offizielleFristenAus(foerderung);
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

  // Sanfte Obergrenzen im Client (freundlicher Hinweis). Die echte
  // Spam-Bremse sitzt später serverseitig (Etappe 3/4).
  const EIGENE_CAP = 200;
  const MELDUNG_CAP = 50;

  // Eigene (selbst recherchierte) Förderung anlegen und direkt auf die
  // Merkliste setzen. Liegt verschlüsselt im Projekt (Tresor).
  async function eigeneFoerderungAnlegen(eingabe) {
    if ((aktivesProjekt.eigeneFoerderungen ?? []).length >= EIGENE_CAP) {
      return {
        ok: false,
        fehler: `Du hast in diesem Projekt schon sehr viele eigene Förderungen (Grenze ${EIGENE_CAP}). Bitte räume zuerst auf.`,
      };
    }
    const f = {
      id: "eigen-" + neueId(),
      eigen: true,
      name: eingabe.name.trim(),
      foerdergeber: eingabe.foerdergeber.trim(),
      land: eingabe.land || "ANDERES",
      beschreibung: eingabe.beschreibung.trim(),
      webseite: eingabe.webseite.trim(),
      foerderhoehe_text: eingabe.foerderhoehe.trim() || "—",
      max_anteil_prozent:
        eingabe.maxAnteil === "" || eingabe.maxAnteil == null
          ? null
          : Number(eingabe.maxAnteil),
      anteil_ausnahme: !!eingabe.anteilAusnahme,
      einreichung_online: !!eingabe.einreichOnline,
      einreich_url: (eingabe.einreichUrl ?? "").trim(),
      frist_hinweis: (eingabe.fristHinweis ?? "").trim(),
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
        zeitpunkt: eingabe.zeitpunkt ?? "fristen",
      },
    };
    aktivesProjekt.eigeneFoerderungen.push(f);
    if (!aktivesProjekt.merkliste.includes(f.id)) {
      aktivesProjekt.merkliste.push(f.id);
    }
    await tresorSpeichern();
    return { ok: true };
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

  // --- Projekt-Auswahlmenü (Wechseln + Umbenennen/Löschen je Projekt) ---
  async function projektWaehlen(id) {
    daten.aktivesProjektId = id;
    projektMenuOffen = false;
    await tresorSpeichern();
  }
  function umbenennenOeffnenFuer(p) {
    daten.aktivesProjektId = p.id;
    umbenennenName = p.name;
    umbenennenOffen = true;
    projektMenuOffen = false;
  }
  function loeschenOeffnenFuer(p) {
    daten.aktivesProjektId = p.id;
    loeschDialogOffen = true;
    projektMenuOffen = false;
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
      <h1 class="logo-gross" aria-label="Antrag 3000">
        <svg viewBox="0 0 470 120" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <svg x="6" y="10" width="100" height="100" viewBox="19.75 9.5 89.5 89.5">
            <g fill="#17b6a6">
              <rect x="30" y="88" width="62" height="7" rx="3" transform="rotate(0.5 61 91.5)"></rect>
              <rect x="27" y="78.5" width="66" height="7" rx="3" transform="rotate(-1.5 60 82)" opacity="0.9"></rect>
              <rect x="33" y="69" width="70" height="7" rx="3" transform="rotate(1 68 72.5)"></rect>
              <rect x="31" y="59.5" width="58" height="7" rx="3" transform="rotate(2 60 63)" opacity="0.92"></rect>
              <rect x="26" y="50" width="66" height="7" rx="3" transform="rotate(-1 59 53.5)"></rect>
              <rect x="34" y="40.5" width="68" height="7" rx="3" transform="rotate(1.5 68 44)" opacity="0.9"></rect>
              <rect x="30" y="31" width="56" height="7" rx="3" transform="rotate(-2.5 58 34.5)"></rect>
              <rect x="29" y="22" width="62" height="7" rx="3" transform="rotate(1.5 60 25.5)" opacity="0.94"></rect>
              <rect x="35" y="13.5" width="52" height="7" rx="3" transform="rotate(4 61 17)"></rect>
            </g>
          </svg>
          <text x="122" y="82" font-family="'DM Serif Display', Georgia, 'Times New Roman', serif" font-size="60" fill="#123a5e">Antrag <tspan fill="#14b7a8">3000</tspan></text>
        </svg>
      </h1>
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
      <h1 class="logo-gross" aria-label="Antrag 3000">
        <svg viewBox="0 0 470 120" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <svg x="6" y="10" width="100" height="100" viewBox="19.75 9.5 89.5 89.5">
            <g fill="#17b6a6">
              <rect x="30" y="88" width="62" height="7" rx="3" transform="rotate(0.5 61 91.5)"></rect>
              <rect x="27" y="78.5" width="66" height="7" rx="3" transform="rotate(-1.5 60 82)" opacity="0.9"></rect>
              <rect x="33" y="69" width="70" height="7" rx="3" transform="rotate(1 68 72.5)"></rect>
              <rect x="31" y="59.5" width="58" height="7" rx="3" transform="rotate(2 60 63)" opacity="0.92"></rect>
              <rect x="26" y="50" width="66" height="7" rx="3" transform="rotate(-1 59 53.5)"></rect>
              <rect x="34" y="40.5" width="68" height="7" rx="3" transform="rotate(1.5 68 44)" opacity="0.9"></rect>
              <rect x="30" y="31" width="56" height="7" rx="3" transform="rotate(-2.5 58 34.5)"></rect>
              <rect x="29" y="22" width="62" height="7" rx="3" transform="rotate(1.5 60 25.5)" opacity="0.94"></rect>
              <rect x="35" y="13.5" width="52" height="7" rx="3" transform="rotate(4 61 17)"></rect>
            </g>
          </svg>
          <text x="122" y="82" font-family="'DM Serif Display', Georgia, 'Times New Roman', serif" font-size="60" fill="#123a5e">Antrag <tspan fill="#14b7a8">3000</tspan></text>
        </svg>
      </h1>
      <p class="untertitel">Tresor entsperren</p>

      {#if merkenAktiv}
        <button type="button" disabled={beschaeftigt} onclick={merkenEntsperrenJetzt}>
          🔓 Mit Windows-Konto entsperren
        </button>
        {#if fehler}<p class="fehler">{fehler}</p>{/if}
        <p class="untertitel" style="margin-top:14px">oder mit Passwort:</p>
      {/if}

      <form onsubmit={entsperren}>
        <label for="pw">Passwort</label>
        <input id="pw" type="password" bind:value={passwort} autocomplete="current-password" />

        {#if merkenMoeglich}
          <label class="merken">
            <input type="checkbox" bind:checked={merkenWunsch} />
            Auf diesem Gerät merken (künftig ohne Passwort starten)
          </label>
        {/if}

        {#if fehler && !merkenAktiv}<p class="fehler">{fehler}</p>{/if}

        <button type="submit" disabled={beschaeftigt || passwort.length === 0}>
          {beschaeftigt ? "Wird geprüft …" : "Entsperren"}
        </button>
      </form>

      {#if merkenAktiv}
        <button class="leise" onclick={merkenVergessen}>
          Gerät vergessen (künftig wieder Passwort verlangen)
        </button>
      {/if}
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
        <span class="logo" role="img" aria-label="Antrag 3000">
          <svg viewBox="0 0 470 120" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
            <svg x="6" y="10" width="100" height="100" viewBox="19.75 9.5 89.5 89.5">
              <g fill="#17b6a6">
                <rect x="30" y="88" width="62" height="7" rx="3" transform="rotate(0.5 61 91.5)"></rect>
                <rect x="27" y="78.5" width="66" height="7" rx="3" transform="rotate(-1.5 60 82)" opacity="0.9"></rect>
                <rect x="33" y="69" width="70" height="7" rx="3" transform="rotate(1 68 72.5)"></rect>
                <rect x="31" y="59.5" width="58" height="7" rx="3" transform="rotate(2 60 63)" opacity="0.92"></rect>
                <rect x="26" y="50" width="66" height="7" rx="3" transform="rotate(-1 59 53.5)"></rect>
                <rect x="34" y="40.5" width="68" height="7" rx="3" transform="rotate(1.5 68 44)" opacity="0.9"></rect>
                <rect x="30" y="31" width="56" height="7" rx="3" transform="rotate(-2.5 58 34.5)"></rect>
                <rect x="29" y="22" width="62" height="7" rx="3" transform="rotate(1.5 60 25.5)" opacity="0.94"></rect>
                <rect x="35" y="13.5" width="52" height="7" rx="3" transform="rotate(4 61 17)"></rect>
              </g>
            </svg>
            <text x="122" y="82" font-family="'DM Serif Display', Georgia, 'Times New Roman', serif" font-size="60" fill="#123a5e">Antrag <tspan fill="#14b7a8">3000</tspan></text>
          </svg>
        </span>
        <div class="projektwahl">
          {#if daten.projekte.length === 0}
            <button class="leise" onclick={() => (neuesProjektOffen = true)}>
              + Erstes Projekt erstellen
            </button>
          {:else}
            <div class="projekt-menu">
              <button
                class="projekt-knopf"
                onclick={() => (projektMenuOffen = !projektMenuOffen)}
                aria-haspopup="true"
                aria-expanded={projektMenuOffen}
                title="Projekt wechseln, umbenennen oder löschen"
              >
                {aktivesProjekt?.name ?? "Projekt wählen"}<span class="pfeil">▾</span>
              </button>
              {#if projektMenuOffen}
                <div class="menu-backdrop" onclick={() => (projektMenuOffen = false)} role="presentation"></div>
                <div class="projekt-liste" role="menu">
                  {#each daten.projekte as p (p.id)}
                    <div class="projekt-zeile" class:aktiv={p.id === daten.aktivesProjektId}>
                      <button class="projekt-name" onclick={() => projektWaehlen(p.id)}>
                        {p.name}
                      </button>
                      <button class="zeile-icon" title="Umbenennen" aria-label={`Projekt „${p.name}" umbenennen`} onclick={() => umbenennenOeffnenFuer(p)}>✏️</button>
                      <button class="zeile-icon" title="Löschen" aria-label={`Projekt „${p.name}" löschen`} onclick={() => loeschenOeffnenFuer(p)}>🗑</button>
                    </div>
                  {/each}
                  <button class="projekt-neu" onclick={() => { projektMenuOffen = false; neuesProjektOffen = true; }}>
                    + Neues Projekt
                  </button>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>
      <nav>
        <div class="modus-schalter" role="group" aria-label="Modus wählen">
          <button class:aktiv={arbeitsModus === "antrag"} onclick={() => arbeitsModusWechseln("antrag")}>
            Antrag
          </button>
          <button class:aktiv={arbeitsModus === "abrechnung"} onclick={() => arbeitsModusWechseln("abrechnung")}>
            Abrechnung
          </button>
        </div>
        <span class="nav-trenner" aria-hidden="true"></span>
        {#if arbeitsModus === "antrag"}
          <button class:aktiv={bereich === "foerderungen"} onclick={() => (bereich = "foerderungen")}>
            Förderungen
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
        {:else}
          <button class:aktiv={bereich === "belege"} onclick={() => (bereich = "belege")}>
            Belege{#if aktivesProjekt?.abrechnung?.belege?.length}&nbsp;({aktivesProjekt.abrechnung.belege.length}){/if}
          </button>
          <button class:aktiv={bereich === "kostenstellen"} onclick={() => (bereich = "kostenstellen")}>
            Kostenstellen
          </button>
          <button class:aktiv={bereich === "geldquellen"} onclick={() => (bereich = "geldquellen")}>
            Geldquellen
          </button>
          <button class:aktiv={bereich === "zuteilung"} onclick={() => (bereich = "zuteilung")}>
            Zuteilung
          </button>
          <button class:aktiv={bereich === "sachbericht"} onclick={() => (bereich = "sachbericht")}>
            Sachbericht
          </button>
        {/if}
        <span class="nav-trenner" aria-hidden="true"></span>
        <button class:aktiv={bereich === "stammdaten"} onclick={() => (bereich = "stammdaten")}>
          Stammdaten &amp; Team
        </button>
      </nav>
      <div class="rechts">
        <button class="leise" onclick={() => (sicherungOffen = true)}>🛡 Sicherung</button>
        <button class="leise" onclick={() => (updateOffen = true)} title="Nach App-Updates suchen">⬆ Update</button>
        <button class="leise" onclick={sperren}>Sperren</button>
        <span
          class="status-punkt {verbindungsStatus.klasse}"
          title={verbindungsStatus.text}
          role="img"
          aria-label={verbindungsStatus.text}
        ></span>
      </div>
    </header>
    <main>
      {#if bereich === "foerderungen"}
        <div class="unter-reiter">
          <button class:aktiv={foerderAnsicht === "alle"} onclick={() => (foerderAnsicht = "alle")}>
            Alle Förderungen
          </button>
          <button class:aktiv={foerderAnsicht === "passend"} onclick={() => (foerderAnsicht = "passend")}>
            Passende für mich
          </button>
        </div>
        {#if foerderAnsicht === "alle"}
          <Foerderungen
            merkliste={aktivesProjekt?.merkliste ?? null}
            umschalten={merklisteUmschalten}
            oeffneKatalog={() => (katalogOffen = true)}
            standFuer={katalogStandFuer}
            neuFelderFuer={katalogNeuFelder}
            {logoHerunterladen}
          />
        {:else if !aktivesProjekt}
          <div class="leer-projekt">
            <div class="karte">
              <h1>Noch kein Projekt</h1>
              <p class="untertitel">
                „Passende für mich" gehört zu einem Projekt – mit eigenem Fragebogen.
                Erstelle zuerst ein Projekt.
              </p>
              <button onclick={() => (neuesProjektOffen = true)}>Projekt erstellen</button>
            </div>
          </div>
        {:else}
          {#key daten.aktivesProjektId}
            <Matching
              antworten={aktivesProjekt?.fragebogen ?? null}
              speichern={fragebogenSpeichern}
              merkliste={aktivesProjekt.merkliste}
              umschalten={merklisteUmschalten}
              oeffneKatalog={() => (katalogOffen = true)}
              standFuer={katalogStandFuer}
              neuFelderFuer={katalogNeuFelder}
              {logoHerunterladen}
            />
          {/key}
        {/if}
      {:else if bereich === "stammdaten"}
        <div class="konto-seite">
          <div class="konto-spalte konto-links">
            <Stammdaten stammdaten={daten.stammdaten} speichern={stammdatenSpeichern} />
          </div>
          <div class="konto-spalte konto-rechts">
            <div class="modus-wahl">
              <span class="modus-titel">Betriebsart</span>
              <div class="modus-knoepfe">
                <button class:aktiv={daten.modus === "einzel"} onclick={() => modusWechseln("einzel")}>
                  Einzelplatz
                </button>
                <button class:aktiv={daten.modus === "team"} onclick={() => modusWechseln("team")}>
                  Team
                </button>
              </div>
            </div>

            {#if daten.modus === "einzel"}
              <div class="einzel-panel">
                <h2>Einzelplatz</h2>
                <p class="dezent">
                  Du nutzt Antrag 3000 allein. <strong>App-Updates</strong> und die
                  <strong>Förder-Datenbank</strong> erhältst du vom Server – ganz ohne
                  Zugangs-Paket. Status, Fristen und eigene Förderer bleiben rein lokal
                  (keine Synchronisation).
                </p>
                <div class="einzel-knoepfe">
                  <button class="primaer" onclick={einzelKatalogHolen}>
                    Förder-Datenbank aktualisieren
                  </button>
                </div>
                <button class="verwaltung-toggle klein-toggle" onclick={() => (selfHostOffen = !selfHostOffen)}>
                  {selfHostOffen ? "▾" : "▸"} Erweitert (Self-Hosting)
                </button>
                {#if selfHostOffen}
                  <p class="dezent klein">
                    Die Server-Adresse ist fest eingebacken (Antrag 3000). Nur
                    ändern, wenn du einen <strong>eigenen Server</strong>
                    betreibst.
                  </p>
                  <label for="einzel-server">Update-Server (Adresse)</label>
                  <input
                    id="einzel-server"
                    type="text"
                    bind:value={daten.einzelServer}
                    onchange={tresorSpeichern}
                    placeholder="https://sync.antrag3000.de"
                  />
                {/if}
                {#if einzelMeldung}<p class="einzel-meldung">{einzelMeldung}</p>{/if}
                <p class="dezent klein">
                  App-Updates findest du oben rechts unter „⬆ Update" (wird beim Start
                  automatisch geprüft).
                </p>
              </div>
            {:else}
            <TeamSync
              sync={daten.sync}
              laden={zugangspaketLaden}
              testen={verbindungPruefen}
              entfernen={zugangspaketEntfernen}
              einladungAnnehmen={einladungAnnehmen}
              mitgliedEinladen={mitgliedEinladen}
              mitgliederHolen={mitgliederHolen}
              mitgliedStatusSetzen={mitgliedStatusSetzen}
              teamErstellen={teamErstellen}
              standardEnrollUrl={daten.einzelServer ?? ""}
              standardSyncAdresse={STANDARD_TEAM_SYNC}
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
            {/if}
          </div>
        </div>
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
      {:else if bereich === "belege"}
        {#key daten.aktivesProjektId}
          <Abrechnung
            belege={aktivesProjekt.abrechnung.belege}
            speichern={belegeSpeichern}
            projektName={aktivesProjekt.name}
            kfp={aktivesProjekt.kfp}
            {kostenstelleAnlegen}
            dateiHinzufuegen={belegDateiHinzufuegen}
            dateiOeffnen={belegDateiOeffnen}
            dateiHerunterladen={belegDateiHerunterladen}
            dateiEntfernen={belegDateiEntfernen}
            ordnerEntfernen={belegOrdnerEntfernen}
          />
        {/key}
      {:else if bereich === "kostenstellen"}
        {#key daten.aktivesProjektId}
          <Kostenstellen
            kfp={aktivesProjekt.kfp}
            belege={aktivesProjekt.abrechnung.belege}
            projektName={aktivesProjekt.name}
          />
        {/key}
      {:else if bereich === "geldquellen"}
        {#key daten.aktivesProjektId}
          <Geldquellen
            quellen={aktivesProjekt.abrechnung.quellen}
            speichern={quellenSpeichern}
            entfernen={quelleEntfernen}
            kfp={aktivesProjekt.kfp}
            belege={aktivesProjekt.abrechnung.belege}
            projektName={aktivesProjekt.name}
          />
        {/key}
      {:else if bereich === "zuteilung"}
        {#key daten.aktivesProjektId}
          <Zuteilung
            belege={aktivesProjekt.abrechnung.belege}
            quellen={aktivesProjekt.abrechnung.quellen}
            speichern={belegeSpeichern}
            kfp={aktivesProjekt.kfp}
            projektName={aktivesProjekt.name}
            nachweisPdf={(id) => nachweisExport(id, "pdf")}
            nachweisWord={(id) => nachweisExport(id, "word")}
          />
        {/key}
      {:else if bereich === "sachbericht"}
        {#key daten.aktivesProjektId}
          <Sachbericht
            sachbericht={aktivesProjekt.abrechnung.sachbericht}
            speichern={sachberichtSpeichern}
            projektName={aktivesProjekt.name}
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
          teamBoard={daten.sync?.teamBoard ?? null}
          {meineProjektIds}
          foerderungLabel={boardFoerderungLabel}
          {logoHerunterladen}
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
          oeffneKatalog={() => (katalogOffen = true)}
          aktualisierteIds={aktivesProjekt.katalogAktualisiert ?? []}
          hinweisVerwerfen={katalogHinweisVerwerfen}
          standFuer={katalogStandFuer}
          neuFelderFuer={katalogNeuFelder}
          {logoHerunterladen}
        />
      {/if}
    </main>

    {#if sicherungOffen}
      <Sicherung schliessen={() => (sicherungOffen = false)} {nachWiederherstellung} />
    {/if}

    {#if updateOffen}
      <UpdatePruefung schliessen={() => (updateOffen = false)} merkeUpdate={updateHinweisMerken} />
    {/if}

    {#if wasIstNeu}
      <div class="schleier-neu" role="presentation" onclick={() => (wasIstNeu = null)}>
        <div class="dialog-neu" role="presentation" onclick={(e) => e.stopPropagation()}>
          <h2>Was ist neu in {wasIstNeu.version}?</h2>
          {#if wasIstNeu.notes}
            <div class="neu-notizen">{wasIstNeu.notes}</div>
          {:else}
            <p style="color:var(--text-muted)">Die App wurde auf Version {wasIstNeu.version} aktualisiert.</p>
          {/if}
          <div class="neu-fuss">
            <button onclick={() => (wasIstNeu = null)}>Verstanden</button>
          </div>
        </div>
      </div>
    {/if}

    {#if katalogOffen}
      <KatalogUpdate
        schliessen={() => (katalogOffen = false)}
        updateAusDatei={katalogUpdateAusDatei}
        updateVomServer={katalogUpdateVomServer}
        syncEingerichtet={katalogServerBereit}
        zuruecksetzen={katalogZuruecksetzen}
        meldungen={daten.katalogMeldungen ?? []}
        {meldungAnlegen}
        {meldungEntfernen}
      />
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
    background: var(--flaeche-2);
    color: var(--text);
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
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm), 0 8px 24px var(--schatten-xs);
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
    color: var(--text-muted);
    font-size: 0.95rem;
    line-height: 1.5;
  }
  .warntext {
    color: var(--gefahr-text);
  }
  /* Projekt-Auswahlmenü (mit Umbenennen/Löschen je Projekt) */
  .projekt-menu {
    position: relative;
    display: inline-block;
  }
  .projekt-knopf {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    width: auto;
    margin: 0;
    padding: 7px 12px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
    max-width: 280px;
  }
  .projekt-knopf:hover {
    border-color: var(--akzent);
  }
  .projekt-knopf .pfeil {
    color: var(--text-muted);
    font-size: 0.8rem;
  }
  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
  }
  .projekt-liste {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 31;
    min-width: 240px;
    max-width: 340px;
    background: var(--weiss);
    border: 1px solid var(--rand);
    border-radius: 10px;
    box-shadow: 0 8px 28px var(--schatten-md);
    padding: 6px;
  }
  .projekt-zeile {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: 7px;
  }
  .projekt-zeile.aktiv {
    background: var(--akzent-rand3);
  }
  .projekt-zeile:hover {
    background: var(--lila-bg5);
  }
  .projekt-name {
    flex: 1 1 auto;
    width: auto;
    margin: 0;
    text-align: left;
    padding: 8px 10px;
    font-size: 0.92rem;
    font-family: inherit;
    color: var(--text);
    background: none;
    border: none;
    border-radius: 7px;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .zeile-icon {
    flex: 0 0 auto;
    width: auto;
    margin: 0;
    padding: 6px 8px;
    font-size: 0.9rem;
    /* Explizite Farbe, damit einfarbig gezeichnete Emoji (z. B. 🗑) nicht
       die geerbte weiße Button-Textfarbe bekommen und unsichtbar werden. */
    color: var(--text-2);
    background: none;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    opacity: 0.7;
  }
  .zeile-icon:hover {
    opacity: 1;
    background: var(--weiss);
  }
  .projekt-neu {
    width: 100%;
    text-align: left;
    margin-top: 4px;
    padding: 8px 10px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--akzent);
    background: none;
    border: none;
    border-top: 1px solid var(--flaeche-2b);
    border-radius: 0 0 7px 7px;
    cursor: pointer;
  }
  .projekt-neu:hover {
    background: var(--lila-bg5);
  }

  /* Unter-Umschalter im Reiter „Förderungen" (Alle / Passende) */
  .unter-reiter {
    display: inline-flex;
    gap: 4px;
    margin-bottom: 18px;
    padding: 4px;
    background: var(--flaeche-3b);
    border-radius: 9px;
  }
  .unter-reiter button {
    width: auto;
    margin: 0;
    padding: 7px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text-muted);
    background: none;
    border: none;
    border-radius: 6px;
    cursor: pointer;
  }
  .unter-reiter button.aktiv {
    background: var(--weiss);
    color: var(--text);
    box-shadow: 0 1px 3px var(--schatten-md2);
  }

  /* Betriebsart-Wähler (Einzelplatz / Team) */
  .modus-wahl {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
  }
  .modus-titel {
    font-size: 0.8rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--text-muted);
  }
  .modus-knoepfe {
    display: inline-flex;
    border: 2px solid var(--rand);
    border-radius: 8px;
    overflow: hidden;
  }
  .modus-knoepfe button {
    border: none;
    background: var(--weiss);
    padding: 7px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text-muted);
    cursor: pointer;
  }
  .modus-knoepfe button.aktiv {
    background: var(--akzent);
    color: var(--weiss);
  }
  .einzel-panel {
    background: var(--weiss);
    border: 1px solid var(--rand);
    border-radius: 12px;
    padding: 20px 22px;
  }
  .einzel-panel h2 {
    margin: 0 0 10px;
    font-size: 1.1rem;
  }
  .einzel-panel label {
    display: block;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-muted);
    margin: 14px 0 4px;
  }
  .einzel-panel input {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 11px;
    font-size: 0.95rem;
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
  }
  .einzel-knoepfe {
    margin-top: 14px;
  }
  .einzel-panel .primaer {
    width: auto;
    margin: 0;
    padding: 9px 16px;
    font-size: 0.92rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--weiss);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .einzel-panel .primaer:hover {
    background: var(--akzent-d);
  }
  .einzel-meldung {
    margin-top: 12px;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text);
  }
  .dezent {
    color: var(--text-muted);
    font-size: 0.92rem;
    line-height: 1.55;
  }
  .dezent.klein {
    font-size: 0.82rem;
  }

  /* Häkchen "Auf diesem Gerät merken" auf dem Entsperr-Bildschirm */
  .merken {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 14px 0 4px;
    font-size: 0.88rem;
    color: var(--text-muted);
    line-height: 1.4;
    cursor: pointer;
  }
  /* Natives kleines Kontrollkästchen – nicht das große Eingabefeld-Styling
     (kein 2px-Rahmen, kein Polster, kein Übergangs-„Aufblinken" beim Anwählen). */
  .merken input[type="checkbox"] {
    appearance: auto;
    width: 16px;
    height: 16px;
    flex: 0 0 auto;
    margin: 1px 0 0;
    padding: 0;
    border: none;
    border-radius: 0;
    background: none;
    transition: none;
    accent-color: var(--akzent);
    cursor: pointer;
  }
  .merken input[type="checkbox"]:focus-visible {
    outline: 2px solid var(--akzent);
    outline-offset: 2px;
  }

  label {
    display: block;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-muted);
    margin: 16px 0 6px;
  }

  input {
    width: 100%;
    box-sizing: border-box;
    padding: 10px 12px;
    font-size: 1rem;
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
    transition: border-color 0.15s, background 0.15s;
  }
  input:focus {
    outline: none;
    border-color: var(--akzent);
    background: var(--weiss);
  }

  button {
    width: 100%;
    margin-top: 20px;
    padding: 11px;
    font-size: 1rem;
    font-weight: 600;
    color: var(--weiss);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s;
  }
  button:hover:not(:disabled) {
    background: var(--akzent-d);
  }
  button:disabled {
    background: var(--grau-3);
    cursor: default;
  }

  button.gefahr {
    background: var(--gefahr);
  }
  button.gefahr:hover:not(:disabled) {
    background: var(--gefahr-text);
  }

  button.leise {
    background: none;
    color: var(--text-muted);
    font-weight: 400;
    font-size: 0.875rem;
    margin-top: 16px;
    padding: 6px;
  }
  button.leise:hover {
    background: none;
    color: var(--text);
    text-decoration: underline;
  }

  .fehler {
    margin: 14px 0 0;
    color: var(--gefahr-text);
    font-size: 0.9rem;
  }

  .warnung {
    margin: 24px 0 0;
    padding: 12px 14px;
    background: var(--warnung-bg);
    border-radius: 8px;
    font-size: 0.85rem;
    line-height: 1.5;
    color: var(--warnung-text2);
  }

  /* Ansicht nach dem Entsperren */
  .app header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 24px;
    background: var(--weiss);
    box-shadow: 0 1px 2px var(--schatten-xs);
  }
  .logo {
    display: flex;
    align-items: center;
  }
  .logo > svg {
    height: 30px;
    width: auto;
    display: block;
  }
  .logo-gross {
    display: flex;
    justify-content: center;
    margin: 0 0 6px;
  }
  .logo-gross > svg {
    height: 56px;
    width: auto;
  }
  .schleier-neu {
    position: fixed; inset: 0; background: var(--schatten-xl);
    display: grid; place-items: center; padding: 24px; z-index: 60;
  }
  .dialog-neu {
    background: var(--weiss); border-radius: 12px; padding: 28px 32px;
    max-width: 460px; width: 100%; box-shadow: 0 12px 40px var(--schatten-lg);
  }
  .dialog-neu h2 { margin: 0 0 14px; font-size: 1.2rem; }
  .neu-notizen { white-space: pre-wrap; line-height: 1.55; font-size: 0.95rem; color: var(--text); }
  .neu-fuss { display: flex; justify-content: flex-end; margin-top: 20px; }
  .neu-fuss button {
    padding: 9px 18px; font-size: 0.92rem; font-weight: 600; font-family: inherit;
    color: var(--weiss); background: var(--akzent); border: none; border-radius: 8px; cursor: pointer;
  }
  .neu-fuss button:hover { background: var(--akzent-d); }
  .app header button.leise {
    width: auto;
    margin: 0;
  }
  .rechts {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .status-punkt {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    margin-left: 6px;
    flex: 0 0 auto;
    cursor: default;
    box-shadow: 0 0 0 3px var(--schatten-schwach);
  }
  .status-punkt.rot { background: var(--gefahr); }
  .status-punkt.blau { background: var(--akzent); }
  .status-punkt.orange { background: var(--warnung-3); }
  .status-punkt.gruen { background: var(--erfolg); }

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
    color: var(--text-2);
    background: none;
    border-radius: 8px;
  }
  nav button:hover:not(.aktiv) {
    background: var(--flaeche-2b);
    color: var(--text);
  }
  nav button.aktiv {
    background: var(--akzent-bg);
    color: var(--akzent-d);
    font-weight: 600;
  }
  /* trennt die drei Navigations-Blöcke */
  .nav-trenner {
    width: 1px;
    align-self: center;
    height: 22px;
    background: var(--rand);
    margin: 0 6px;
  }

  /* Oberster Modus-Umschalter (Antrag | Abrechnung): segmentierter Schalter,
     deutlich abgesetzt von den normalen Reitern. */
  .modus-schalter {
    display: inline-flex;
    align-items: center;
    background: var(--flaeche-2c);
    border-radius: 9px;
    padding: 2px;
    gap: 2px;
  }
  .modus-schalter button {
    padding: 6px 14px;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-2);
    background: none;
    border-radius: 7px;
  }
  .modus-schalter button:hover:not(.aktiv) {
    background: var(--flaeche-3c);
    color: var(--text);
  }
  .modus-schalter button.aktiv {
    background: var(--akzent);
    color: var(--weiss);
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
    color: var(--text);
    background: var(--flaeche-c);
    border: 2px solid var(--rand);
    border-radius: 8px;
    max-width: 220px;
  }
  .projektwahl select:focus {
    outline: none;
    border-color: var(--akzent);
  }
  .projektwahl button.leise {
    width: auto;
    margin: 0;
  }

  .schleier {
    position: fixed;
    inset: 0;
    background: var(--schatten-xl);
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

  /* Zusammengeführter Reiter „Stammdaten & Team": zwei Spalten
     nebeneinander (links Stammdaten, rechts Synchronisation), mit einer
     feinen Trennlinie. Auf schmalen Fenstern untereinander. */
  .konto-seite {
    display: flex;
    align-items: stretch;
  }
  .konto-spalte {
    flex: 1 1 0;
    min-width: 0;
  }
  .konto-rechts {
    border-left: 1px solid var(--rand-2);
  }
  @media (max-width: 900px) {
    .konto-seite {
      flex-direction: column;
    }
    .konto-rechts {
      border-left: none;
      border-top: 1px solid var(--rand-2);
    }
  }
</style>
