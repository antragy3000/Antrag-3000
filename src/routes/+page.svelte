<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Foerderungen from "$lib/komponenten/Foerderungen.svelte";
  import Matching from "$lib/komponenten/Matching.svelte";
  import Merkliste from "$lib/komponenten/Merkliste.svelte";
  import Stammdaten from "$lib/komponenten/Stammdaten.svelte";

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

    // Projekte aus aelteren Staenden bekommen eine leere Merkliste.
    for (const p of d.projekte) {
      if (!Array.isArray(p.merkliste)) {
        p.merkliste = [];
        veraendert = true;
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
    if (d.version !== 2) {
      d.version = 2;
      veraendert = true;
    }
    return veraendert;
  }

  onMount(async () => {
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
    await invoke("tresor_sperren");
    daten = null;
    fehler = "";
    bereich = "alle";
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

  // Förderung auf die Merkliste des aktiven Projekts setzen bzw.
  // wieder entfernen (Stern-Knopf).
  async function merklisteUmschalten(id) {
    if (!aktivesProjekt) {
      neuesProjektOffen = true;
      return;
    }
    const liste = aktivesProjekt.merkliste;
    aktivesProjekt.merkliste = liste.includes(id)
      ? liste.filter((x) => x !== id)
      : [...liste, id];
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
        <button class:aktiv={bereich === "merkliste"} onclick={() => (bereich = "merkliste")}>
          Merkliste{#if aktivesProjekt?.merkliste.length}&nbsp;({aktivesProjekt.merkliste.length}){/if}
        </button>
        <button class:aktiv={bereich === "stammdaten"} onclick={() => (bereich = "stammdaten")}>
          Stammdaten
        </button>
      </nav>
      <button class="leise" onclick={sperren}>Sperren</button>
    </header>
    <main>
      {#if bereich === "alle"}
        <Foerderungen
          merkliste={aktivesProjekt?.merkliste ?? null}
          umschalten={merklisteUmschalten}
        />
      {:else if bereich === "stammdaten"}
        <Stammdaten stammdaten={daten.stammdaten} speichern={stammdatenSpeichern} />
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
      {:else}
        <Merkliste
          merkliste={aktivesProjekt.merkliste}
          umschalten={merklisteUmschalten}
          {ordnerOeffnen}
        />
      {/if}
    </main>

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
