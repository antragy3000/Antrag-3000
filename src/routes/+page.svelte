<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Foerderungen from "$lib/komponenten/Foerderungen.svelte";

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

  // Struktur eines frischen Tresors (wächst in späteren Schritten).
  const LEERER_TRESOR = {
    version: 1,
    stammdaten: {},
    projekte: [],
  };

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
      await invoke("tresor_erstellen", {
        passwort,
        daten: JSON.stringify(LEERER_TRESOR),
      });
      daten = structuredClone(LEERER_TRESOR);
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
    ansicht = "entsperren";
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
      <span class="logo">Antrag 3000</span>
      <button class="leise" onclick={sperren}>Sperren</button>
    </header>
    <main>
      <Foerderungen />
    </main>
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
</style>
